use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};

use crate::model::*;
use crate::playtime::merged_secs;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("unknown store id in database: {0}")]
    BadStore(String),
    #[error("That file isn't a Hearth backup.")]
    NotABackup,
    #[error("That backup comes from a newer version of Hearth. Update Hearth first.")]
    BackupTooNew,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, DbError>;

/// Ordered schema migrations; index + 1 is the `user_version` it produces.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_init.sql"),
    include_str!("../migrations/002_details.sql"),
    include_str!("../migrations/003_active_sessions.sql"),
    include_str!("../migrations/004_art_source.sql"),
    include_str!("../migrations/005_saves.sql"),
    include_str!("../migrations/006_status.sql"),
    include_str!("../migrations/007_launch_options.sql"),
];

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        let version: usize = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version) {
            conn.execute_batch(sql)?;
            conn.pragma_update(None, "user_version", i + 1)?;
        }
        Ok(Self { conn })
    }

    /// Writes a complete, consistent copy of the library to `dest` (safe while
    /// Hearth runs: SQLite takes a snapshot), with `meta` such as the game
    /// folders and appearance settings stored inside it. Sign-ins are never in
    /// the library, so never in a backup.
    pub fn backup_to(&self, dest: &Path, meta: &[(&str, &str)]) -> Result<()> {
        if dest.exists() {
            std::fs::remove_file(dest)?;
        }
        self.conn.execute("VACUUM INTO ?1", [dest.to_string_lossy()])?;
        let copy = Connection::open(dest)?;
        copy.execute_batch("CREATE TABLE backup_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);")?;
        for (k, v) in meta {
            copy.execute("INSERT INTO backup_meta (key, value) VALUES (?1, ?2)", [k, v])?;
        }
        Ok(())
    }

    /// How many rows Hearth has written since the library was opened: a cheap
    /// way to tell whether anything changed since the last automatic backup.
    pub fn changes(&self) -> u64 {
        self.conn.total_changes()
    }

    /// Drops a restored backup's settings table; the library doesn't need it.
    pub fn forget_backup_meta(&self) -> Result<()> {
        self.conn.execute_batch("DROP TABLE IF EXISTS backup_meta;")?;
        Ok(())
    }

    /// The settings stored in a backup, after checking it is one Hearth can open.
    pub fn read_backup(path: &Path) -> Result<HashMap<String, String>> {
        let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| DbError::NotABackup)?;
        let version: usize = c.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|_| DbError::NotABackup)?;
        if version == 0 {
            return Err(DbError::NotABackup);
        }
        if version > MIGRATIONS.len() {
            return Err(DbError::BackupTooNew);
        }
        let mut stmt = c.prepare("SELECT key, value FROM backup_meta").map_err(|_| DbError::NotABackup)?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let meta: HashMap<String, String> = rows.collect::<rusqlite::Result<_>>()?;
        // The library itself must be there too.
        c.query_row("SELECT COUNT(*) FROM games", [], |r| r.get::<_, i64>(0)).map_err(|_| DbError::NotABackup)?;
        Ok(meta)
    }

    /// Apply one provider's results. Entries the store no longer reports are
    /// kept (they may simply need a login to show up) but marked uninstalled.
    pub fn sync_store(&mut self, store: StoreId, entries: &[GameEntry], now: i64) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE store_entries SET installed = 0 WHERE store = ?1",
            [store.as_str()],
        )?;
        for e in entries {
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT id FROM store_entries WHERE store = ?1 AND store_game_id = ?2",
                    params![store.as_str(), e.store_game_id],
                    |r| r.get(0),
                )
                .optional()?;
            match existing {
                Some(id) => {
                    tx.execute(
                        // Stores like Epic report a total but no date: when the
                        // total grows between syncs, the game was played since.
                        "UPDATE store_entries SET installed = ?2, install_dir = ?3,
                            store_last_played = COALESCE(?5,
                                CASE WHEN store_playtime_secs IS NOT NULL AND ?4 > store_playtime_secs
                                     THEN ?6 ELSE store_last_played END),
                            store_playtime_secs = ?4,
                            store_baseline_secs = COALESCE(store_baseline_secs, ?4)
                         WHERE id = ?1",
                        params![
                            id,
                            e.installed,
                            e.install_dir,
                            e.store_playtime_secs,
                            e.store_last_played,
                            now
                        ],
                    )?;
                }
                None => {
                    let norm = normalize_title(&e.title);
                    let game_id: Option<i64> = tx
                        .query_row("SELECT id FROM games WHERE norm_title = ?1", [&norm], |r| {
                            r.get(0)
                        })
                        .optional()?;
                    let game_id = match game_id {
                        Some(id) => id,
                        None => {
                            tx.execute(
                                "INSERT INTO games (title, norm_title, added_at) VALUES (?1, ?2, ?3)",
                                params![e.title, norm, now],
                            )?;
                            tx.last_insert_rowid()
                        }
                    };
                    tx.execute(
                        "INSERT INTO store_entries (game_id, store, store_game_id, installed,
                            install_dir, store_playtime_secs, store_baseline_secs, store_last_played)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7)",
                        params![
                            game_id,
                            store.as_str(),
                            e.store_game_id,
                            e.installed,
                            e.install_dir,
                            e.store_playtime_secs,
                            e.store_last_played
                        ],
                    )?;
                }
            }
        }
        // A game the store no longer reports, on a drive that IS connected, was
        // uninstalled: forget its folder. On a drive that isn't connected, keep
        // it, so the game shows as "drive not connected" until the drive is back.
        let stale: Vec<(i64, String)> = {
            let mut stmt = tx.prepare(
                "SELECT id, install_dir FROM store_entries
                 WHERE store = ?1 AND installed = 0 AND install_dir IS NOT NULL",
            )?;
            let rows = stmt.query_map([store.as_str()], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let mut drives = Drives::default();
        for (id, dir) in stale {
            if drives.present(&dir) {
                tx.execute("UPDATE store_entries SET install_dir = NULL WHERE id = ?1", [id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Hide or unhide every store entry of a game. Stored by (store, id) so a
    /// re-sync never brings a hidden game back.
    pub fn set_game_hidden(&mut self, game_id: i64, hidden: bool) -> Result<()> {
        let tx = self.conn.transaction()?;
        let keys: Vec<(String, String)> = {
            let mut stmt =
                tx.prepare("SELECT store, store_game_id FROM store_entries WHERE game_id = ?1")?;
            let rows = stmt.query_map([game_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for (store, id) in keys {
            if hidden {
                tx.execute(
                    "INSERT OR IGNORE INTO hidden (store, store_game_id) VALUES (?1, ?2)",
                    params![store, id],
                )?;
            } else {
                tx.execute(
                    "DELETE FROM hidden WHERE store = ?1 AND store_game_id = ?2",
                    params![store, id],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Replace the saved running sessions: (game id, started, last seen).
    pub fn checkpoint_active(&mut self, active: &[(i64, i64, i64)]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM active_sessions", [])?;
        for (game, started, seen) in active {
            tx.execute(
                "INSERT OR REPLACE INTO active_sessions (game_id, started_at, last_seen) VALUES (?1, ?2, ?3)",
                params![game, started, seen],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Sessions that were running when Hearth last stopped; reading removes them.
    pub fn take_active(&mut self) -> Result<Vec<(i64, i64, i64)>> {
        let tx = self.conn.transaction()?;
        let rows: Vec<(i64, i64, i64)> = {
            let mut stmt = tx.prepare("SELECT game_id, started_at, last_seen FROM active_sessions")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        tx.execute("DELETE FROM active_sessions", [])?;
        tx.commit()?;
        Ok(rows)
    }

    /// Install folders of installed games from every store except `except`.
    pub fn store_install_dirs(&self, except: StoreId) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT install_dir FROM store_entries
             WHERE installed = 1 AND install_dir IS NOT NULL AND store != ?1",
        )?;
        let rows = stmt.query_map([except.as_str()], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Every game's id, most recently played first (so their details come first).
    pub fn game_ids(&self) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT g.id FROM games g
              ORDER BY (SELECT MAX(e.store_last_played) FROM store_entries e WHERE e.game_id = g.id) DESC NULLS LAST, g.id",
        )?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Games with no cover yet, with one of their store entries.
    pub fn games_missing_cover(&self) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare("SELECT id FROM games WHERE cover IS NULL")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Games whose artwork comes (or should come) from a Steam store page:
    /// (game id, Steam app id). Covers from other stores are left alone.
    pub fn steam_art_targets(&self) -> Result<Vec<(i64, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT g.id, CAST(e.store_game_id AS INTEGER) FROM games g
               JOIN store_entries e ON e.game_id = g.id AND e.store = 'steam'
              WHERE (g.cover IS NULL OR g.cover LIKE '%steamstatic.com%')
                AND COALESCE(g.art_source, 'store') = 'store'
             UNION
             SELECT g.id, json_extract(g.details_json, '$.steam_app_id') FROM games g
              WHERE g.details_json IS NOT NULL
                AND g.id NOT IN (SELECT game_id FROM store_entries WHERE store = 'steam')
                AND (g.cover IS NULL OR g.cover LIKE '%steamstatic.com%')
                AND COALESCE(g.art_source, 'store') = 'store'",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Games still on store artwork that SteamGridDB hasn't been asked about
    /// since `older_than`: (id, title, Steam app id if known). Installed and
    /// recently played games come first.
    pub fn sgdb_targets(&self, older_than: i64, limit: i64) -> Result<Vec<(i64, String, Option<i64>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT g.id, g.title,
                    COALESCE((SELECT CAST(e.store_game_id AS INTEGER) FROM store_entries e
                               WHERE e.game_id = g.id AND e.store = 'steam' LIMIT 1),
                             json_extract(g.details_json, '$.steam_app_id'))
               FROM games g
              WHERE COALESCE(g.art_source, 'store') = 'store'
                AND (g.sgdb_checked_at IS NULL OR g.sgdb_checked_at < ?1)
              ORDER BY (SELECT MAX(installed) FROM store_entries e WHERE e.game_id = g.id) DESC,
                       (SELECT MAX(store_last_played) FROM store_entries e WHERE e.game_id = g.id) DESC
              LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![older_than, limit], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn mark_sgdb_checked(&self, game_id: i64, now: i64) -> Result<()> {
        self.conn
            .execute("UPDATE games SET sgdb_checked_at = ?2 WHERE id = ?1", params![game_id, now])?;
        Ok(())
    }

    /// Set one piece of artwork. `kind` is "cover", "hero" or "logo";
    /// `source` is "store", "sgdb" or "user". Hand-picked art is never
    /// replaced by an automatic source.
    pub fn set_art(&self, game_id: i64, kind: &str, url: &str, source: &str) -> Result<bool> {
        let column = match kind {
            "cover" => "cover",
            "hero" => "hero",
            "logo" => "logo",
            _ => return Ok(false),
        };
        let sql = format!(
            "UPDATE games SET {column} = ?2, art_source = ?3
              WHERE id = ?1 AND (?3 = 'user' OR COALESCE(art_source, 'store') != 'user')"
        );
        Ok(self.conn.execute(&sql, params![game_id, url, source])? > 0)
    }

    pub fn set_status(&self, game_id: i64, status: Option<GameStatus>) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET status = ?2 WHERE id = ?1",
            params![game_id, status.map(GameStatus::as_str)],
        )?;
        Ok(())
    }

    /// The user's collections with how many visible games each holds.
    pub fn collections(&self) -> Result<Vec<Collection>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.name, COUNT(cg.game_id) FROM collections c
               LEFT JOIN collection_games cg ON cg.collection_id = c.id
              GROUP BY c.id ORDER BY c.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| Ok(Collection { id: r.get(0)?, name: r.get(1)?, count: r.get(2)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn create_collection(&self, name: &str) -> Result<i64> {
        self.conn.execute("INSERT INTO collections (name) VALUES (?1)", [name.trim()])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn rename_collection(&self, id: i64, name: &str) -> Result<()> {
        self.conn.execute("UPDATE collections SET name = ?2 WHERE id = ?1", params![id, name.trim()])?;
        Ok(())
    }

    /// Deletes the list only; its games stay in the library.
    pub fn delete_collection(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM collections WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn set_in_collection(&self, collection: i64, game: i64, member: bool) -> Result<()> {
        if member {
            self.conn.execute(
                "INSERT OR IGNORE INTO collection_games (collection_id, game_id) VALUES (?1, ?2)",
                params![collection, game],
            )?;
        } else {
            self.conn.execute(
                "DELETE FROM collection_games WHERE collection_id = ?1 AND game_id = ?2",
                params![collection, game],
            )?;
        }
        Ok(())
    }

    /// Games whose save locations haven't been looked up since `older_than`:
    /// (id, title, Steam app id if known).
    pub fn save_lookup_targets(&self, older_than: i64, limit: i64) -> Result<Vec<(i64, String, Option<i64>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT g.id, g.title,
                    COALESCE((SELECT CAST(e.store_game_id AS INTEGER) FROM store_entries e
                               WHERE e.game_id = g.id AND e.store = 'steam' LIMIT 1),
                             json_extract(g.details_json, '$.steam_app_id'))
               FROM games g
              WHERE g.save_checked_at IS NULL OR g.save_checked_at < ?1
              LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![older_than, limit], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_save_paths(&self, game_id: i64, paths: &[String], now: i64) -> Result<()> {
        let json = serde_json::to_string(paths).unwrap_or_else(|_| "[]".into());
        self.conn.execute(
            "UPDATE games SET save_paths_json = ?2, save_checked_at = ?3 WHERE id = ?1",
            params![game_id, json, now],
        )?;
        Ok(())
    }

    /// Games with known save locations: (id, paths, an install folder).
    pub fn save_locations(&self) -> Result<Vec<(i64, Vec<String>, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT g.id, g.save_paths_json,
                    (SELECT install_dir FROM store_entries e WHERE e.game_id = g.id AND e.install_dir IS NOT NULL LIMIT 1)
               FROM games g WHERE g.save_paths_json IS NOT NULL AND g.save_paths_json != '[]'",
        )?;
        let rows = stmt.query_map([], |r| {
            let json: String = r.get(1)?;
            Ok((r.get(0)?, serde_json::from_str(&json).unwrap_or_default(), r.get(2)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_save_seen(&self, game_id: i64, at: i64) -> Result<bool> {
        Ok(self.conn.execute(
            "UPDATE games SET save_seen_at = ?2 WHERE id = ?1 AND (save_seen_at IS NULL OR save_seen_at != ?2)",
            params![game_id, at],
        )? > 0)
    }

    /// Drop chosen artwork so the store's (and SteamGridDB's) apply again.
    pub fn reset_art(&self, game_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET cover = NULL, hero = NULL, logo = NULL, art_source = NULL, sgdb_checked_at = NULL
              WHERE id = ?1",
            [game_id],
        )?;
        Ok(())
    }

    /// Forget lookups that found nothing, so they're tried again (after the
    /// matcher improves, or at startup).
    pub fn clear_detail_misses(&self) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET details_checked_at = NULL WHERE details_json IS NULL",
            [],
        )?;
        Ok(())
    }

    /// Replace a game's artwork (the user picked its store page).
    pub fn set_game_art(&self, game_id: i64, cover: &str, hero: &str, logo: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET cover = ?2, hero = ?3, logo = ?4 WHERE id = ?1",
            params![game_id, cover, hero, logo],
        )?;
        Ok(())
    }

    /// Rewrites saved art addresses that need changing. `fix(url, slot)`
    /// (slot is "cover", "hero" or "logo") returns the new address, or None
    /// to keep it. Returns how many games changed.
    pub fn rewrite_art(&self, fix: impl Fn(&str, &str) -> Option<String>) -> Result<usize> {
        let mut changes = Vec::new();
        {
            let mut stmt = self.conn.prepare("SELECT id, cover, hero, logo FROM games")?;
            let rows = stmt.query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?))
            })?;
            for row in rows {
                let (id, cover, hero, logo) = row?;
                let new = |url: &Option<String>, slot| url.as_deref().and_then(|u| fix(u, slot));
                let (c, h, l) = (new(&cover, "cover"), new(&hero, "hero"), new(&logo, "logo"));
                if c.is_some() || h.is_some() || l.is_some() {
                    changes.push((id, c.or(cover), h.or(hero), l.or(logo)));
                }
            }
        }
        for (id, cover, hero, logo) in &changes {
            self.conn.execute("UPDATE games SET cover = ?2, hero = ?3, logo = ?4 WHERE id = ?1", params![id, cover, hero, logo])?;
        }
        Ok(changes.len())
    }

    /// Set artwork on a game directly (from matched store-page details).
    pub fn set_game_art_if_missing(&self, game_id: i64, cover: &str, hero: &str, logo: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET cover = COALESCE(cover, ?2), hero = COALESCE(hero, ?3), logo = COALESCE(logo, ?4)
             WHERE id = ?1",
            params![game_id, cover, hero, logo],
        )?;
        Ok(())
    }

    /// Cached store-page details: `(json, checked_at)`. `json` is `None` when
    /// a lookup found nothing, so it isn't retried on every click.
    pub fn details(&self, game_id: i64) -> Result<Option<(Option<String>, i64)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT details_json, details_checked_at FROM games WHERE id = ?1 AND details_checked_at IS NOT NULL",
                [game_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    pub fn set_details(&self, game_id: i64, json: Option<&str>, now: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET details_json = ?2, details_checked_at = ?3 WHERE id = ?1",
            params![game_id, json, now],
        )?;
        Ok(())
    }

    pub fn install_size(&self, game_id: i64) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row("SELECT install_size FROM games WHERE id = ?1", [game_id], |r| r.get(0))
            .optional()?
            .flatten())
    }

    pub fn set_install_size(&self, game_id: i64, bytes: i64) -> Result<()> {
        self.conn
            .execute("UPDATE games SET install_size = ?2 WHERE id = ?1", params![game_id, bytes])?;
        Ok(())
    }

    /// Drop store entries that turned out not to be games, and any game left with no entries.
    pub fn remove_entries(&mut self, store: StoreId, store_game_ids: &[&str]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for id in store_game_ids {
            tx.execute(
                "DELETE FROM store_entries WHERE store = ?1 AND store_game_id = ?2",
                params![store.as_str(), id],
            )?;
        }
        tx.execute(
            "DELETE FROM games WHERE id NOT IN (SELECT game_id FROM store_entries)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Fill artwork the store supplies, without overwriting anything already chosen.
    pub fn set_art_if_missing(
        &self,
        store: StoreId,
        store_game_id: &str,
        cover: Option<&str>,
        hero: Option<&str>,
        logo: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET cover = COALESCE(cover, ?3), hero = COALESCE(hero, ?4),
                logo = COALESCE(logo, ?5)
             WHERE id = (SELECT game_id FROM store_entries WHERE store = ?1 AND store_game_id = ?2)",
            params![store.as_str(), store_game_id, cover, hero, logo],
        )?;
        Ok(())
    }

    /// Adds a way to play (id None) or updates one; returns its id.
    pub fn save_launch_option(
        &self,
        id: Option<i64>,
        game_id: i64,
        name: &str,
        path: &str,
        args: &str,
        working_dir: Option<&str>,
    ) -> Result<i64> {
        let working_dir = working_dir.map(str::trim).filter(|d| !d.is_empty());
        match id {
            Some(id) => {
                self.conn.execute(
                    "UPDATE launch_options SET name = ?2, path = ?3, args = ?4, working_dir = ?5 WHERE id = ?1",
                    params![id, name.trim(), path.trim(), args.trim(), working_dir],
                )?;
                Ok(id)
            }
            None => {
                self.conn.execute(
                    "INSERT INTO launch_options (game_id, name, path, args, working_dir) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![game_id, name.trim(), path.trim(), args.trim(), working_dir],
                )?;
                Ok(self.conn.last_insert_rowid())
            }
        }
    }

    pub fn delete_launch_option(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM launch_options WHERE id = ?1", [id])?;
        Ok(())
    }

    /// What Play does for a game: one of its ways to play, or None for the store.
    pub fn set_default_launch(&self, game_id: i64, option: Option<i64>) -> Result<()> {
        self.conn.execute(
            "UPDATE launch_options SET is_default = (id IS ?2) WHERE game_id = ?1",
            params![game_id, option],
        )?;
        Ok(())
    }

    pub fn launch_option(&self, id: i64) -> Result<Option<LaunchOption>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, game_id, name, path, args, working_dir, is_default FROM launch_options WHERE id = ?1",
                [id],
                launch_option_row,
            )
            .optional()?)
    }

    pub fn set_favorite(&self, game_id: i64, favorite: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE games SET favorite = ?2 WHERE id = ?1",
            params![game_id, favorite],
        )?;
        Ok(())
    }

    pub fn record_session(
        &self,
        game_id: i64,
        started_at: i64,
        ended_at: i64,
        source: SessionSource,
    ) -> Result<i64> {
        let source = match source {
            SessionSource::Hub => "hub",
            SessionSource::Tracker => "tracker",
        };
        self.conn.execute(
            "INSERT INTO sessions (game_id, started_at, ended_at, source) VALUES (?1, ?2, ?3, ?4)",
            params![game_id, started_at, ended_at, source],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Every recorded play session of games that aren't hidden, oldest first
    /// (the Stats page's history).
    pub fn sessions(&self) -> Result<Vec<PlaySession>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.game_id, s.started_at, s.ended_at, s.source FROM sessions s
             WHERE EXISTS (
                 SELECT 1 FROM store_entries e
                 WHERE e.game_id = s.game_id
                   AND NOT EXISTS (SELECT 1 FROM hidden h WHERE h.store = e.store AND h.store_game_id = e.store_game_id))
             ORDER BY s.started_at",
        )?;
        let rows = stmt.query_map([], |r| {
            let source: String = r.get(4)?;
            Ok(PlaySession {
                id: r.get(0)?,
                game_id: r.get(1)?,
                started_at: r.get(2)?,
                ended_at: r.get(3)?,
                source: if source == "hub" { SessionSource::Hub } else { SessionSource::Tracker },
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Install folders of installed games, for the tracker to match processes against.
    pub fn install_dirs(&self) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT game_id, install_dir FROM store_entries
             WHERE installed = 1 AND install_dir IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn game(&self, game_id: i64) -> Result<Option<Game>> {
        Ok(self.load_games(Some(game_id))?.into_iter().next())
    }

    pub fn list_games(&self, q: &LibraryQuery) -> Result<Vec<Game>> {
        let search = q
            .search
            .as_deref()
            .map(normalize_title)
            .filter(|s| !s.is_empty());
        let mut games: Vec<Game> = self
            .load_games(None)?
            .into_iter()
            .filter(|g| g.hidden == q.hidden_only)
            .filter(|g| match q.install {
                InstallFilter::All => true,
                InstallFilter::Installed => g.installed,
                InstallFilter::NotInstalled => !g.installed,
            })
            .filter(|g| q.stores.is_empty() || g.entries.iter().any(|e| q.stores.contains(&e.store)))
            .filter(|g| q.status.is_none_or(|s| g.status == Some(s)))
            .filter(|g| q.collection.is_none_or(|c| g.collections.contains(&c)))
            .filter(|g| {
                search
                    .as_ref()
                    .is_none_or(|s| normalize_title(&g.title).contains(s.as_str()))
            })
            .collect();
        games.sort_by_cached_key(|g| g.title.to_lowercase());
        Ok(games)
    }

    fn load_games(&self, only: Option<i64>) -> Result<Vec<Game>> {
        let mut entries: HashMap<i64, Vec<(StoreEntry, Option<i64>)>> = HashMap::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT e.id, e.game_id, e.store, e.store_game_id, e.installed, e.install_dir,
                        h.store IS NOT NULL, e.store_playtime_secs, e.store_baseline_secs,
                        e.store_last_played
                 FROM store_entries e
                 LEFT JOIN hidden h ON h.store = e.store AND h.store_game_id = e.store_game_id
                 WHERE ?1 IS NULL OR e.game_id = ?1
                 ORDER BY e.id",
            )?;
            let mut rows = stmt.query([only])?;
            while let Some(r) = rows.next()? {
                let store: String = r.get(2)?;
                let entry = StoreEntry {
                    id: r.get(0)?,
                    game_id: r.get(1)?,
                    store: StoreId::parse(&store).ok_or(DbError::BadStore(store))?,
                    store_game_id: r.get(3)?,
                    installed: r.get(4)?,
                    install_dir: r.get(5)?,
                    hidden: r.get(6)?,
                    store_playtime_secs: r.get(7)?,
                    store_baseline_secs: r.get(8)?,
                    offline: false,
                };
                entries.entry(entry.game_id).or_default().push((entry, r.get(9)?));
            }
        }

        let mut sessions: HashMap<i64, (i64, i64)> = HashMap::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT game_id, SUM(ended_at - started_at), MAX(ended_at) FROM sessions
                 WHERE ?1 IS NULL OR game_id = ?1 GROUP BY game_id",
            )?;
            let rows = stmt.query_map([only], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?;
            for row in rows {
                let (id, v) = row?;
                sessions.insert(id, v);
            }
        }

        let mut in_collections: HashMap<i64, Vec<i64>> = HashMap::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT game_id, collection_id FROM collection_games WHERE ?1 IS NULL OR game_id = ?1 ORDER BY collection_id",
            )?;
            let rows = stmt.query_map([only], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
            for row in rows {
                let (g, c) = row?;
                in_collections.entry(g).or_default().push(c);
            }
        }

        let mut ways: HashMap<i64, Vec<LaunchOption>> = HashMap::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT id, game_id, name, path, args, working_dir, is_default FROM launch_options
                 WHERE ?1 IS NULL OR game_id = ?1 ORDER BY id",
            )?;
            for row in stmt.query_map([only], launch_option_row)? {
                let o = row?;
                ways.entry(o.game_id).or_default().push(o);
            }
        }

        let mut stmt = self.conn.prepare(
            "SELECT id, title, favorite, added_at, cover, hero, logo, description, save_seen_at, status FROM games
             WHERE ?1 IS NULL OR id = ?1",
        )?;
        let rows = stmt.query_map([only], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, bool>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<i64>>(8)?,
                r.get::<_, Option<String>>(9)?,
            ))
        })?;

        let mut drives = Drives::default();
        let mut games = Vec::new();
        for row in rows {
            let (id, title, favorite, added_at, cover, hero, logo, description, save_seen, status) = row?;
            let with_last = entries.remove(&id).unwrap_or_default();
            let (hub_secs, hub_last) = sessions.get(&id).copied().unwrap_or((0, 0));

            let store_total = with_last
                .iter()
                .filter_map(|(e, _)| e.store_playtime_secs)
                .reduce(|a, b| a + b);
            let baseline = with_last
                .iter()
                .filter_map(|(e, _)| e.store_baseline_secs)
                .reduce(|a, b| a + b);
            let last_played = with_last
                .iter()
                .filter_map(|(_, l)| *l)
                .chain((hub_last > 0).then_some(hub_last))
                // The newest save file dates sessions nobody else saw.
                .chain(save_seen)
                .max();
            let entries: Vec<StoreEntry> = with_last
                .into_iter()
                .map(|(mut e, _)| {
                    e.offline = e.install_dir.as_deref().is_some_and(|d| !drives.present(d));
                    e
                })
                .collect();
            let installed = entries.iter().any(|e| e.installed && !e.offline);
            let offline_drive = if installed {
                None
            } else {
                entries.iter().find(|e| e.offline).and_then(|e| e.install_dir.as_deref()).map(drive_label)
            };

            games.push(Game {
                id,
                title,
                favorite,
                hidden: !entries.is_empty() && entries.iter().all(|e| e.hidden),
                installed,
                added_at,
                last_played,
                playtime_secs: merged_secs(store_total, baseline, hub_secs),
                cover,
                hero,
                logo,
                description,
                entries,
                status: status.as_deref().and_then(GameStatus::parse),
                collections: in_collections.remove(&id).unwrap_or_default(),
                offline_drive,
                launch_options: ways.remove(&id).unwrap_or_default(),
            });
        }
        Ok(games)
    }
}

fn launch_option_row(r: &rusqlite::Row) -> rusqlite::Result<LaunchOption> {
    Ok(LaunchOption {
        id: r.get(0)?,
        game_id: r.get(1)?,
        name: r.get(2)?,
        path: r.get(3)?,
        args: r.get(4)?,
        working_dir: r.get(5)?,
        is_default: r.get(6)?,
    })
}

/// The drive (or network share) a path lives on: "E:\\Games\\X" -> "E:\\".
fn drive_root(dir: &str) -> PathBuf {
    Path::new(dir).ancestors().last().map(Path::to_path_buf).unwrap_or_default()
}

/// "E:" for a path on drive E:, the share for a network path.
fn drive_label(dir: &str) -> String {
    drive_root(dir).to_string_lossy().trim_end_matches(['\\', '/']).to_string()
}

/// Which drives are connected, asked once per drive however many games use it.
#[derive(Default)]
struct Drives(HashMap<PathBuf, bool>);

impl Drives {
    fn present(&mut self, dir: &str) -> bool {
        let root = drive_root(dir);
        // A relative or rootless path can't be on a missing drive.
        if root.as_os_str().is_empty() || !root.has_root() {
            return true;
        }
        *self.0.entry(root.clone()).or_insert_with(|| root.exists())
    }
}

/// Lowercased alphanumerics only, so "DOOM Eternal™" and "Doom Eternal" match.
pub fn normalize_title(title: &str) -> String {
    title
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(store: StoreId, id: &str, title: &str, installed: bool) -> GameEntry {
        GameEntry {
            store,
            store_game_id: id.into(),
            title: title.into(),
            installed,
            install_dir: installed.then(|| format!("C:/Games/{id}")),
            store_playtime_secs: None,
            store_last_played: None,
        }
    }

    #[test]
    fn ways_to_play_are_saved_edited_defaulted_and_removed() {
        let mut db = Db::open_in_memory().unwrap();
        db.sync_store(StoreId::Steam, &[entry(StoreId::Steam, "1", "HITMAN World of Assassination", true)], 0)
            .unwrap();
        let game = db.list_games(&LibraryQuery::default()).unwrap()[0].id;
        let peacock = db
            .save_launch_option(None, game, " Peacock (offline) ", "C:/Peacock/Start Server.cmd", "", Some("  "))
            .unwrap();
        let g = db.game(game).unwrap().unwrap();
        assert_eq!(g.launch_options.len(), 1);
        assert_eq!(g.launch_options[0].name, "Peacock (offline)");
        assert_eq!(g.launch_options[0].working_dir, None, "a blank folder is no folder");
        assert!(!g.launch_options[0].is_default, "the store stays the default until chosen");

        db.set_default_launch(game, Some(peacock)).unwrap();
        assert!(db.launch_option(peacock).unwrap().unwrap().is_default);
        db.save_launch_option(Some(peacock), game, "Peacock", "C:/Peacock/Start Server.cmd", "-offline", None)
            .unwrap();
        let o = db.launch_option(peacock).unwrap().unwrap();
        assert_eq!((o.name.as_str(), o.args.as_str(), o.is_default), ("Peacock", "-offline", true));

        db.set_default_launch(game, None).unwrap();
        assert!(!db.launch_option(peacock).unwrap().unwrap().is_default);
        db.delete_launch_option(peacock).unwrap();
        assert!(db.game(game).unwrap().unwrap().launch_options.is_empty());
    }

    #[test]
    fn same_title_on_two_stores_merges_into_one_game() {
        let mut db = Db::open_in_memory().unwrap();
        db.sync_store(StoreId::Steam, &[entry(StoreId::Steam, "1", "DOOM Eternal", true)], 0)
            .unwrap();
        db.sync_store(StoreId::Epic, &[entry(StoreId::Epic, "a", "Doom Eternal™", false)], 0)
            .unwrap();
        let games = db.list_games(&LibraryQuery::default()).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].entries.len(), 2);
        assert!(games[0].installed);
    }

    #[test]
    fn hidden_game_stays_hidden_after_resync() {
        let mut db = Db::open_in_memory().unwrap();
        let freebie = [entry(StoreId::Epic, "x", "Some Freebie", false)];
        db.sync_store(StoreId::Epic, &freebie, 0).unwrap();
        let id = db.list_games(&LibraryQuery::default()).unwrap()[0].id;
        db.set_game_hidden(id, true).unwrap();
        db.sync_store(StoreId::Epic, &freebie, 10).unwrap();

        assert!(db.list_games(&LibraryQuery::default()).unwrap().is_empty());
        let hidden = LibraryQuery { hidden_only: true, ..Default::default() };
        assert_eq!(db.list_games(&hidden).unwrap().len(), 1);

        db.set_game_hidden(id, false).unwrap();
        assert_eq!(db.list_games(&LibraryQuery::default()).unwrap().len(), 1);
    }

    #[test]
    fn filters_by_install_state_store_and_search() {
        let mut db = Db::open_in_memory().unwrap();
        db.sync_store(
            StoreId::Steam,
            &[
                entry(StoreId::Steam, "1", "Hades", true),
                entry(StoreId::Steam, "2", "Celeste", false),
            ],
            0,
        )
        .unwrap();
        db.sync_store(StoreId::Epic, &[entry(StoreId::Epic, "a", "Control", true)], 0)
            .unwrap();

        let q = |install, stores: Vec<StoreId>, search: Option<&str>| LibraryQuery {
            install,
            stores,
            search: search.map(Into::into),
            ..Default::default()
        };
        let titles = |q: LibraryQuery| -> Vec<String> {
            db.list_games(&q).unwrap().into_iter().map(|g| g.title).collect()
        };
        assert_eq!(titles(q(InstallFilter::Installed, vec![], None)), ["Control", "Hades"]);
        assert_eq!(titles(q(InstallFilter::NotInstalled, vec![], None)), ["Celeste"]);
        assert_eq!(titles(q(InstallFilter::All, vec![StoreId::Epic], None)), ["Control"]);
        assert_eq!(titles(q(InstallFilter::All, vec![], Some("ade"))), ["Hades"]);
    }

    #[test]
    fn playtime_merges_store_total_with_sessions() {
        let mut db = Db::open_in_memory().unwrap();
        let mut e = entry(StoreId::Steam, "1", "Hades", true);
        e.store_playtime_secs = Some(3600);
        db.sync_store(StoreId::Steam, &[e.clone()], 0).unwrap();
        let id = db.list_games(&LibraryQuery::default()).unwrap()[0].id;

        db.record_session(id, 1000, 2800, SessionSource::Tracker).unwrap();
        assert_eq!(db.game(id).unwrap().unwrap().playtime_secs, 5400);
        assert_eq!(db.game(id).unwrap().unwrap().last_played, Some(2800));

        // Steam catches up and now includes our 30 minutes: still 1h30m, not 2h.
        e.store_playtime_secs = Some(5400);
        db.sync_store(StoreId::Steam, &[e], 3000).unwrap();
        assert_eq!(db.game(id).unwrap().unwrap().playtime_secs, 5400);
    }

    /// A drive letter that isn't connected on this machine.
    fn missing_drive() -> String {
        ('D'..='Z')
            .map(|c| format!("{c}:\\"))
            .find(|d| !Path::new(d).exists())
            .expect("a free drive letter")
    }

    #[test]
    fn games_on_a_missing_drive_are_not_connected() {
        let mut db = Db::open_in_memory().unwrap();
        let gone = missing_drive();
        let mut e = entry(StoreId::Epic, "a", "Control", true);
        e.install_dir = Some(format!("{gone}Games\\Control"));
        db.sync_store(StoreId::Epic, &[e], 0).unwrap();
        let g = &db.list_games(&LibraryQuery::default()).unwrap()[0];
        assert!(!g.installed, "a store still says installed, but the drive is gone");
        assert!(g.entries[0].offline);
        assert_eq!(g.offline_drive.as_deref(), Some(gone.trim_end_matches('\\')));

        // The game folder's drive is gone and the scan no longer finds it:
        // its folder is kept, so it stays "not connected" rather than uninstalled.
        db.sync_store(StoreId::Epic, &[], 1).unwrap();
        let g = &db.list_games(&LibraryQuery::default()).unwrap()[0];
        assert!(g.offline_drive.is_some());
    }

    #[test]
    fn uninstalled_games_on_a_present_drive_forget_their_folder() {
        let mut db = Db::open_in_memory().unwrap();
        let here = std::env::temp_dir().join("hearth-test-game").to_string_lossy().into_owned();
        let mut e = entry(StoreId::Steam, "1", "Hades", true);
        e.install_dir = Some(here);
        db.sync_store(StoreId::Steam, &[e], 0).unwrap();
        assert!(db.list_games(&LibraryQuery::default()).unwrap()[0].installed);

        db.sync_store(StoreId::Steam, &[], 1).unwrap();
        let g = &db.list_games(&LibraryQuery::default()).unwrap()[0];
        assert!(!g.installed);
        assert_eq!(g.offline_drive, None, "an ordinary uninstall, not a missing drive");
        assert_eq!(g.entries[0].install_dir, None);
    }

    #[test]
    fn backups_round_trip_with_statuses_and_settings() {
        let dir = std::env::temp_dir().join(format!("hearth-backup-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (live, file) = (dir.join("live.db"), dir.join("test.hearthbackup"));
        let mut db = Db::open(&live).unwrap();
        db.sync_store(StoreId::Steam, &[entry(StoreId::Steam, "1", "Hades", true)], 0).unwrap();
        let id = db.list_games(&LibraryQuery::default()).unwrap()[0].id;
        db.set_status(id, Some(GameStatus::Backlog)).unwrap();
        db.record_session(id, 100, 400, SessionSource::Tracker).unwrap();

        db.backup_to(&file, &[("game_folders", "[\"D:\\\\Games\"]")]).unwrap();
        let meta = Db::read_backup(&file).unwrap();
        assert_eq!(meta.get("game_folders").map(String::as_str), Some("[\"D:\\\\Games\"]"));

        let restored = Db::open(&file).unwrap();
        let g = restored.game(id).unwrap().unwrap();
        assert_eq!((g.title.as_str(), g.status), ("Hades", Some(GameStatus::Backlog)));
        assert_eq!(restored.sessions().unwrap().len(), 1);

        // Anything else is refused.
        std::fs::write(dir.join("notes.txt"), "not a backup").unwrap();
        assert!(matches!(Db::read_backup(&dir.join("notes.txt")), Err(DbError::NotABackup)));
        drop((db, restored));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sessions_skip_hidden_games() {
        let mut db = Db::open_in_memory().unwrap();
        db.sync_store(
            StoreId::Steam,
            &[entry(StoreId::Steam, "1", "Hades", true), entry(StoreId::Steam, "2", "Celeste", true)],
            0,
        )
        .unwrap();
        let ids: Vec<i64> = db.list_games(&LibraryQuery::default()).unwrap().iter().map(|g| g.id).collect();
        db.record_session(ids[1], 500, 900, SessionSource::Tracker).unwrap();
        db.record_session(ids[0], 100, 400, SessionSource::Hub).unwrap();
        let started: Vec<i64> = db.sessions().unwrap().iter().map(|s| s.started_at).collect();
        assert_eq!(started, [100, 500]);

        db.set_game_hidden(ids[1], true).unwrap();
        let left = db.sessions().unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!((left[0].game_id, left[0].source), (ids[0], SessionSource::Hub));
    }

    #[test]
    fn caches_details_and_install_size() {
        let mut db = Db::open_in_memory().unwrap();
        db.sync_store(StoreId::Steam, &[entry(StoreId::Steam, "1", "Hades", true)], 0).unwrap();
        let id = db.list_games(&LibraryQuery::default()).unwrap()[0].id;

        assert_eq!(db.details(id).unwrap(), None, "never looked up yet");
        db.set_details(id, None, 10).unwrap();
        assert_eq!(db.details(id).unwrap(), Some((None, 10)), "a miss is remembered");
        db.set_details(id, Some(r#"{"genres":["Action"]}"#), 20).unwrap();
        assert_eq!(db.details(id).unwrap().unwrap().0.as_deref(), Some(r#"{"genres":["Action"]}"#));

        assert_eq!(db.install_size(id).unwrap(), None);
        db.set_install_size(id, 1234).unwrap();
        assert_eq!(db.install_size(id).unwrap(), Some(1234));
    }

    #[test]
    fn growing_store_playtime_marks_the_game_as_played() {
        let mut db = Db::open_in_memory().unwrap();
        let mut e = entry(StoreId::Epic, "rdr2", "Red Dead Redemption 2", true);
        e.store_playtime_secs = Some(228_183);
        db.sync_store(StoreId::Epic, &[e.clone()], 100).unwrap();
        let id = db.list_games(&LibraryQuery::default()).unwrap()[0].id;
        assert_eq!(db.game(id).unwrap().unwrap().last_played, None, "first sync says nothing about when");

        db.sync_store(StoreId::Epic, &[e.clone()], 200).unwrap();
        assert_eq!(db.game(id).unwrap().unwrap().last_played, None, "no change, no date");

        e.store_playtime_secs = Some(232_000);
        db.sync_store(StoreId::Epic, &[e], 300).unwrap();
        assert_eq!(db.game(id).unwrap().unwrap().last_played, Some(300));
    }

    #[test]
    fn running_sessions_survive_a_restart() {
        let mut db = Db::open_in_memory().unwrap();
        db.sync_store(StoreId::Manual, &[entry(StoreId::Manual, "x", "Blackwood", true)], 0).unwrap();
        let id = db.list_games(&LibraryQuery::default()).unwrap()[0].id;
        db.checkpoint_active(&[(id, 1000, 1600)]).unwrap();
        assert_eq!(db.take_active().unwrap(), [(id, 1000, 1600)]);
        assert!(db.take_active().unwrap().is_empty(), "taken once");
    }

    #[test]
    fn hand_picked_art_is_never_replaced_automatically() {
        let mut db = Db::open_in_memory().unwrap();
        db.sync_store(StoreId::Steam, &[entry(StoreId::Steam, "620", "Portal 2", true)], 0).unwrap();
        let id = db.list_games(&LibraryQuery::default()).unwrap()[0].id;

        assert_eq!(db.sgdb_targets(100, 10).unwrap(), [(id, "Portal 2".to_string(), Some(620))]);
        assert!(db.set_art(id, "cover", "https://cdn2.steamgriddb.com/grid/a.png", "sgdb").unwrap());
        assert!(db.sgdb_targets(100, 10).unwrap().is_empty(), "already on SteamGridDB art");

        assert!(db.set_art(id, "cover", "mine.png", "user").unwrap());
        assert!(!db.set_art(id, "cover", "https://cdn2.steamgriddb.com/grid/b.png", "sgdb").unwrap());
        assert_eq!(db.game(id).unwrap().unwrap().cover.as_deref(), Some("mine.png"));
        assert!(db.steam_art_targets().unwrap().is_empty(), "Steam refresh leaves it alone too");
    }

    #[test]
    fn save_files_date_games_nobody_saw_played() {
        let mut db = Db::open_in_memory().unwrap();
        db.sync_store(StoreId::Epic, &[entry(StoreId::Epic, "ff", "Freedom Fighters", false)], 0).unwrap();
        let id = db.list_games(&LibraryQuery::default()).unwrap()[0].id;
        assert_eq!(db.save_lookup_targets(100, 10).unwrap(), [(id, "Freedom Fighters".to_string(), None)]);

        db.set_save_paths(id, &["{{p|appdata}}\\IO Interactive\\Freedom Fighters".into()], 200).unwrap();
        assert!(db.save_lookup_targets(100, 10).unwrap().is_empty(), "looked up recently");
        assert_eq!(db.save_locations().unwrap().len(), 1);

        assert!(db.set_save_seen(id, 1_784_294_580).unwrap());
        assert!(!db.set_save_seen(id, 1_784_294_580).unwrap(), "unchanged");
        assert_eq!(db.game(id).unwrap().unwrap().last_played, Some(1_784_294_580));
    }

    #[test]
    fn statuses_and_collections_filter_the_library() {
        let mut db = Db::open_in_memory().unwrap();
        db.sync_store(
            StoreId::Steam,
            &[entry(StoreId::Steam, "1", "Hades", true), entry(StoreId::Steam, "2", "Celeste", true)],
            0,
        )
        .unwrap();
        let all = db.list_games(&LibraryQuery::default()).unwrap();
        let (celeste, hades) = (all[0].id, all[1].id);

        db.set_status(hades, Some(GameStatus::Playing)).unwrap();
        let playing = LibraryQuery { status: Some(GameStatus::Playing), ..Default::default() };
        assert_eq!(db.list_games(&playing).unwrap().len(), 1);
        assert_eq!(db.game(hades).unwrap().unwrap().status, Some(GameStatus::Playing));

        let coop = db.create_collection("  Co-op night ").unwrap();
        db.set_in_collection(coop, celeste, true).unwrap();
        db.set_in_collection(coop, celeste, true).unwrap();
        assert_eq!(db.collections().unwrap(), [Collection { id: coop, name: "Co-op night".into(), count: 1 }]);
        let in_coop = LibraryQuery { collection: Some(coop), ..Default::default() };
        assert_eq!(db.list_games(&in_coop).unwrap()[0].title, "Celeste");
        assert_eq!(db.game(celeste).unwrap().unwrap().collections, [coop]);

        db.delete_collection(coop).unwrap();
        assert!(db.collections().unwrap().is_empty());
        assert_eq!(db.list_games(&LibraryQuery::default()).unwrap().len(), 2, "games stay");
    }

    #[test]
    fn missing_entries_become_uninstalled_not_deleted() {
        let mut db = Db::open_in_memory().unwrap();
        db.sync_store(StoreId::Steam, &[entry(StoreId::Steam, "1", "Hades", true)], 0)
            .unwrap();
        db.sync_store(StoreId::Steam, &[], 1).unwrap();
        let games = db.list_games(&LibraryQuery::default()).unwrap();
        assert_eq!(games.len(), 1);
        assert!(!games[0].installed);
    }
}
