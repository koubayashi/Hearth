use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::model::*;
use crate::playtime::merged_secs;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("unknown store id in database: {0}")]
    BadStore(String),
}

pub type Result<T> = std::result::Result<T, DbError>;

/// Ordered schema migrations; index + 1 is the `user_version` it produces.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_init.sql"),
    include_str!("../migrations/002_details.sql"),
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
                        "UPDATE store_entries SET installed = ?2, install_dir = ?3,
                            store_playtime_secs = ?4, store_last_played = ?5,
                            store_baseline_secs = COALESCE(store_baseline_secs, ?4)
                         WHERE id = ?1",
                        params![
                            id,
                            e.installed,
                            e.install_dir,
                            e.store_playtime_secs,
                            e.store_last_played
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

    /// Install folders of installed games from every store except `except`.
    pub fn store_install_dirs(&self, except: StoreId) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT install_dir FROM store_entries
             WHERE installed = 1 AND install_dir IS NOT NULL AND store != ?1",
        )?;
        let rows = stmt.query_map([except.as_str()], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Games with no cover yet, with one of their store entries.
    pub fn games_missing_cover(&self) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare("SELECT id FROM games WHERE cover IS NULL")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
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

        let mut stmt = self.conn.prepare(
            "SELECT id, title, favorite, added_at, cover, hero, logo, description FROM games
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
            ))
        })?;

        let mut games = Vec::new();
        for row in rows {
            let (id, title, favorite, added_at, cover, hero, logo, description) = row?;
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
                .max();
            let entries: Vec<StoreEntry> = with_last.into_iter().map(|(e, _)| e).collect();

            games.push(Game {
                id,
                title,
                favorite,
                hidden: !entries.is_empty() && entries.iter().all(|e| e.hidden),
                installed: entries.iter().any(|e| e.installed),
                added_at,
                last_played,
                playtime_secs: merged_secs(store_total, baseline, hub_secs),
                cover,
                hero,
                logo,
                description,
                entries,
            });
        }
        Ok(games)
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
            hidden_only: false,
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
