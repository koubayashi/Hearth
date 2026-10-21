//! Steam: installed games and playtime from the local client files, plus every
//! owned game (with Steam's server-side playtime) through the official Web API
//! when the user has added their own API key.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use hub_core::{GameEntry, StoreId};
use hub_launch::LaunchAction;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use crate::{vdf, Artwork, LibraryProvider};

const OWNED_URL: &str = "https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/";

/// Not games: runtimes and tools Steam installs alongside them.
const NOT_GAMES: &[&str] = &["228980", "480", "1070560", "1391110", "1628350", "1493710", "1826330"];

/// How Hearth is allowed to read the account's owned games.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SteamAuth {
    /// A web session token from signing in on Steam's own login page.
    Session { token: String },
    /// A personal Web API key (advanced alternative).
    ApiKey { key: String },
}

impl SteamAuth {
    /// The account a session token belongs to (the JWT's `sub` claim).
    fn steam_id(&self) -> Option<String> {
        match self {
            SteamAuth::Session { token } => jwt_subject(token),
            SteamAuth::ApiKey { .. } => None,
        }
    }
}

pub struct Steam {
    root: Option<PathBuf>,
    auth: Mutex<Option<SteamAuth>>,
    http: reqwest::Client,
}

/// The Steam account signed in on this PC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamAccount {
    pub steam_id: String,
    pub name: String,
}

/// For a signed-in browser session, returns a Web API token. Steam's own
/// store pages use it the same way.
pub const TOKEN_URL: &str = "https://store.steampowered.com/pointssummary/ajaxgetasyncconfig";
/// Steam's login page, returning to [`TOKEN_URL`] once signed in.
pub const LOGIN_URL: &str = "https://store.steampowered.com/login/?redir=pointssummary%2Fajaxgetasyncconfig";

impl Steam {
    /// Locates Steam via the registry, falling back to the default install path.
    pub fn detect(auth: Option<SteamAuth>) -> Self {
        Self::new(steam_root(), auth)
    }

    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self::new(Some(root.into()), None)
    }

    fn new(root: Option<PathBuf>, auth: Option<SteamAuth>) -> Self {
        Self {
            root,
            auth: Mutex::new(auth),
            http: reqwest::Client::builder().user_agent("Hearth/0.1").build().expect("http client"),
        }
    }

    pub fn auth(&self) -> Option<SteamAuth> {
        self.auth.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn set_auth(&self, auth: Option<SteamAuth>) {
        *self.auth.lock().unwrap_or_else(|e| e.into_inner()) = auth;
    }

    /// The account to read: the signed-in session's, else the Steam client's.
    pub fn account(&self) -> Option<SteamAccount> {
        let local = self
            .root
            .as_ref()
            .and_then(|r| std::fs::read_to_string(r.join("config").join("loginusers.vdf")).ok());
        let users = local.as_deref().map(parse_login_users).unwrap_or_default();
        match self.auth().and_then(|a| a.steam_id()) {
            Some(id) => Some(
                users
                    .into_iter()
                    .find(|u| u.steam_id == id)
                    .unwrap_or(SteamAccount { name: "your Steam account".into(), steam_id: id }),
            ),
            None => most_recent(users),
        }
    }

    /// Tries an auth method against the Web API; returns how many games it sees.
    pub async fn check_auth(&self, auth: &SteamAuth) -> anyhow::Result<usize> {
        let steam_id = match auth.steam_id() {
            Some(id) => id,
            None => self.account().context("Sign in to the Steam app on this PC first")?.steam_id,
        };
        Ok(self.fetch_owned(auth, &steam_id).await?.len())
    }

    async fn fetch_owned(&self, auth: &SteamAuth, steam_id: &str) -> anyhow::Result<Vec<OwnedGame>> {
        let cred = match auth {
            SteamAuth::Session { token } => ("access_token", token.as_str()),
            SteamAuth::ApiKey { key } => ("key", key.as_str()),
        };
        let res = self
            .http
            .get(OWNED_URL)
            .query(&[
                cred,
                ("steamid", steam_id),
                ("include_appinfo", "1"),
                ("include_played_free_games", "1"),
                ("format", "json"),
            ])
            .send()
            .await?;
        match (res.status().as_u16(), auth) {
            (200, _) => {}
            (401 | 403, SteamAuth::Session { .. }) => bail!(SESSION_EXPIRED),
            (401 | 403, SteamAuth::ApiKey { .. }) => {
                bail!("Steam rejected the API key. Check it at steamcommunity.com/dev/apikey.")
            }
            (code, _) => bail!("Steam's Web API returned {code}. Try again later."),
        }
        let body: OwnedResponse = res.json().await.context("reading Steam's owned games")?;
        Ok(body.response.games)
    }

    fn library_dirs(&self) -> Vec<PathBuf> {
        let Some(root) = &self.root else { return Vec::new() };
        let mut dirs = vec![root.clone()];
        if let Ok(text) = std::fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")) {
            let doc = vdf::parse(&text);
            if let Some(lf) = doc.get("libraryfolders") {
                for (_, folder) in lf.entries() {
                    if let Some(p) = folder.str("path") {
                        let p = PathBuf::from(p);
                        if !dirs.iter().any(|d| same_path(d, &p)) {
                            dirs.push(p);
                        }
                    }
                }
            }
        }
        dirs
    }

    fn art_urls(app_id: &str) -> Artwork {
        let base = format!("https://shared.cloudflare.steamstatic.com/store_item_assets/steam/apps/{app_id}");
        Artwork {
            cover: Some(format!("{base}/library_600x900.jpg")),
            hero: Some(format!("{base}/library_hero.jpg")),
            logo: Some(format!("{base}/logo.png")),
        }
    }
}

impl Steam {
    /// Playtime the Steam client records for every account that has signed in
    /// on this PC, read from `userdata/<account>/config/localconfig.vdf`.
    /// When several accounts played the same game, the highest total wins.
    fn local_playtime(&self) -> HashMap<String, Played> {
        let mut out: HashMap<String, Played> = HashMap::new();
        let Some(root) = &self.root else { return out };
        let Ok(users) = std::fs::read_dir(root.join("userdata")) else { return out };
        for user in users.flatten() {
            let Ok(text) = std::fs::read_to_string(user.path().join("config").join("localconfig.vdf")) else {
                continue;
            };
            for (id, p) in parse_local_playtime(&text) {
                let cur = out.entry(id).or_insert(Played { secs: 0, last_played: None });
                cur.secs = cur.secs.max(p.secs);
                cur.last_played = cur.last_played.max(p.last_played);
            }
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Played {
    secs: i64,
    last_played: Option<i64>,
}

fn parse_local_playtime(text: &str) -> HashMap<String, Played> {
    let doc = vdf::parse(text);
    let apps = doc
        .get("UserLocalConfigStore")
        .and_then(|d| d.get("Software"))
        .and_then(|d| d.get("Valve"))
        .and_then(|d| d.get("Steam"))
        .and_then(|d| d.get("apps"));
    let Some(apps) = apps else { return HashMap::new() };
    apps.entries()
        .filter_map(|(id, app)| {
            // Steam stores minutes.
            let minutes: i64 = app.str("Playtime")?.parse().ok()?;
            let last_played = app.str("LastPlayed").and_then(|s| s.parse().ok()).filter(|&t: &i64| t > 0);
            Some((id.clone(), Played { secs: minutes * 60, last_played }))
        })
        .collect()
}

#[async_trait]
impl LibraryProvider for Steam {
    fn id(&self) -> StoreId {
        StoreId::Steam
    }

    async fn installed(&self) -> anyhow::Result<Vec<GameEntry>> {
        let mut games = Vec::new();
        for dir in self.library_dirs() {
            let apps = dir.join("steamapps");
            let Ok(read) = std::fs::read_dir(&apps) else { continue };
            for file in read.flatten() {
                let name = file.file_name();
                let name = name.to_string_lossy();
                if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                    continue;
                }
                if let Some(g) = std::fs::read_to_string(file.path())
                    .ok()
                    .and_then(|t| parse_manifest(&t, &apps))
                {
                    games.push(g);
                }
            }
        }
        let played = self.local_playtime();
        for g in &mut games {
            if let Some(p) = played.get(&g.store_game_id) {
                g.store_playtime_secs = Some(p.secs);
                g.store_last_played = g.store_last_played.max(p.last_played);
            }
        }
        Ok(games)
    }

    async fn owned(&self) -> anyhow::Result<Vec<GameEntry>> {
        let Some(auth) = self.auth() else { return Ok(Vec::new()) };
        let Some(account) = self.account() else { return Ok(Vec::new()) };
        let games = self.fetch_owned(&auth, &account.steam_id).await?;
        Ok(games
            .into_iter()
            .filter(|g| !NOT_GAMES.contains(&g.appid.to_string().as_str()))
            .map(|g| GameEntry {
                store: StoreId::Steam,
                store_game_id: g.appid.to_string(),
                title: g.name,
                installed: false,
                install_dir: None,
                // Steam's servers count every PC, so this is the most complete total.
                store_playtime_secs: Some(g.playtime_forever * 60),
                store_last_played: g.rtime_last_played.filter(|&t| t > 0),
            })
            .collect())
    }

    fn excluded_ids(&self) -> &[&str] {
        NOT_GAMES
    }

    fn artwork(&self, store_game_id: &str) -> Artwork {
        Self::art_urls(store_game_id)
    }

    fn launch(&self, store_game_id: &str) -> LaunchAction {
        LaunchAction::Uri { uri: format!("steam://rungameid/{store_game_id}") }
    }

    fn install(&self, store_game_id: &str) -> Option<LaunchAction> {
        Some(LaunchAction::Uri { uri: format!("steam://install/{store_game_id}") })
    }

    fn uninstall(&self, store_game_id: &str) -> Option<LaunchAction> {
        Some(LaunchAction::Uri { uri: format!("steam://uninstall/{store_game_id}") })
    }
}

fn parse_manifest(text: &str, steamapps: &Path) -> Option<GameEntry> {
    let doc = vdf::parse(text);
    let app = doc.get("AppState")?;
    let id = app.str("appid")?.to_string();
    let title = app.str("name")?.to_string();
    if NOT_GAMES.contains(&id.as_str()) || title.contains("Redistributable") || title.starts_with("Proton ") {
        return None;
    }
    // StateFlags bit 4 = fully installed; anything else is downloading/updating.
    let flags: u32 = app.str("StateFlags").and_then(|s| s.parse().ok()).unwrap_or(0);
    let install_dir = app
        .str("installdir")
        .map(|d| steamapps.join("common").join(d).to_string_lossy().into_owned());
    Some(GameEntry {
        store: StoreId::Steam,
        store_game_id: id,
        title,
        installed: flags & 4 != 0,
        install_dir,
        store_playtime_secs: None,
        store_last_played: app.str("LastPlayed").and_then(|s| s.parse().ok()).filter(|&t: &i64| t > 0),
    })
}

#[derive(Deserialize)]
struct OwnedResponse {
    response: OwnedGames,
}

#[derive(Deserialize, Default)]
struct OwnedGames {
    #[serde(default)]
    games: Vec<OwnedGame>,
}

#[derive(Deserialize)]
struct OwnedGame {
    appid: u64,
    name: String,
    /// Minutes.
    #[serde(default)]
    playtime_forever: i64,
    rtime_last_played: Option<i64>,
}

/// Shown when a session token stops working; the app offers to sign in again.
pub const SESSION_EXPIRED: &str = "Your Steam sign-in expired. Sign in again in Settings.";

/// Accounts from `loginusers.vdf`, most recent first.
fn parse_login_users(text: &str) -> Vec<SteamAccount> {
    let doc = vdf::parse(text);
    let Some(users) = doc.get("users") else { return Vec::new() };
    let mut out: Vec<(bool, SteamAccount)> = users
        .entries()
        .map(|(id, u)| {
            let name = u.str("PersonaName").or(u.str("AccountName")).unwrap_or(id).to_string();
            (u.str("MostRecent") == Some("1"), SteamAccount { steam_id: id.clone(), name })
        })
        .collect();
    out.sort_by_key(|(recent, _)| !recent);
    out.into_iter().map(|(_, a)| a).collect()
}

fn most_recent(users: Vec<SteamAccount>) -> Option<SteamAccount> {
    users.into_iter().next()
}

/// The `sub` claim of a JWT, without verifying it (Steam verifies it).
fn jwt_subject(token: &str) -> Option<String> {
    use base64::Engine;
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    v.get("sub")?.as_str().map(str::to_string)
}

/// Pulls the token out of the JSON that [`TOKEN_URL`] returns. None when
/// the browser session isn't signed in.
pub fn token_from_body(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body.trim()).ok()?;
    let t = v.get("data")?.get("webapi_token")?.as_str()?;
    (!t.is_empty()).then(|| t.to_string())
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    norm(a) == norm(b)
}

#[cfg(windows)]
fn steam_root() -> Option<PathBuf> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Valve\Steam")
        .and_then(|k| k.get_value::<String, _>("SteamPath"))
        .ok()
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(r"C:\Program Files (x86)\Steam")))
        .filter(|p| p.join("steamapps").is_dir())
}

#[cfg(not(windows))]
fn steam_root() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#""AppState"
    {
        "appid"		"1145350"
        "name"		"Hades II"
        "StateFlags"		"4"
        "installdir"		"Hades II"
        "LastPlayed"		"1792400000"
    }"#;

    #[test]
    fn parses_installed_manifest() {
        let g = parse_manifest(MANIFEST, Path::new(r"D:\SteamLibrary\steamapps")).unwrap();
        assert_eq!(g.store_game_id, "1145350");
        assert_eq!(g.title, "Hades II");
        assert!(g.installed);
        assert_eq!(g.install_dir.as_deref(), Some(r"D:\SteamLibrary\steamapps\common\Hades II"));
        assert_eq!(g.store_last_played, Some(1792400000));
    }

    #[test]
    fn updating_game_is_not_ready() {
        let m = MANIFEST.replace(r#""StateFlags"		"4""#, r#""StateFlags"		"1026""#);
        assert!(!parse_manifest(&m, Path::new("x")).unwrap().installed);
    }

    #[test]
    fn reads_playtime_from_local_config() {
        let text = r#""UserLocalConfigStore" { "Software" { "Valve" { "Steam" { "apps" {
            "2488620" { "LastPlayed" "1792200761" "Playtime" "8237" "Playtime2wks" "103" }
            "241100" { "autocloud" { "lastlaunch" "1792194722" } }
        } } } } }"#;
        let p = parse_local_playtime(text);
        assert_eq!(p["2488620"], Played { secs: 8237 * 60, last_played: Some(1792200761) });
        assert!(!p.contains_key("241100"), "apps without a Playtime value are skipped");
    }

    #[test]
    fn picks_most_recent_login_and_parses_owned_games() {
        let users = r#""users" {
            "76561190000000001" { "AccountName" "old" "PersonaName" "Old" "MostRecent" "0" }
            "76561190000000042" { "AccountName" "player_one" "PersonaName" "ExamplePersona" "MostRecent" "1" }
        }"#;
        assert_eq!(
            most_recent(parse_login_users(users)),
            Some(SteamAccount { steam_id: "76561190000000042".into(), name: "ExamplePersona".into() })
        );

        let body: OwnedResponse = serde_json::from_str(
            r#"{"response":{"game_count":1,"games":[{"appid":2488620,"name":"F1® 24","playtime_forever":9800,"rtime_last_played":1792404453}]}}"#,
        )
        .unwrap();
        let g = &body.response.games[0];
        assert_eq!((g.appid, g.name.as_str(), g.playtime_forever), (2488620, "F1® 24", 9800));
        // A private profile answers with an empty response object.
        let empty: OwnedResponse = serde_json::from_str(r#"{"response":{}}"#).unwrap();
        assert!(empty.response.games.is_empty());
    }

    #[test]
    fn reads_session_token_and_its_account() {
        // header.payload.signature with payload {"sub":"76561190000000042","aud":["web"]}
        let token = "eyJhbGciOiJFZERTQSJ9.eyJzdWIiOiI3NjU2MTE5MDAwMDAwMDA0MiIsImF1ZCI6WyJ3ZWIiXX0.sig";
        let body = format!(r#"{{"success":1,"data":{{"webapi_token":"{token}"}}}}"#);
        assert_eq!(token_from_body(&body).as_deref(), Some(token));
        assert_eq!(SteamAuth::Session { token: token.into() }.steam_id().as_deref(), Some("76561190000000042"));
        assert_eq!(token_from_body(r#"{"success":1,"data":{"webapi_token":""}}"#), None);
    }

    #[test]
    fn skips_redistributables() {
        let m = r#""AppState" { "appid" "228980" "name" "Steamworks Common Redistributables" "StateFlags" "4" }"#;
        assert!(parse_manifest(m, Path::new("x")).is_none());
    }

    #[tokio::test]
    async fn scans_a_library_tree() {
        let root = std::env::temp_dir().join(format!("hearth-steam-test-{}", std::process::id()));
        let apps = root.join("steamapps");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(apps.join("appmanifest_1145350.acf"), MANIFEST).unwrap();
        std::fs::write(apps.join("libraryfolders.vdf"), r#""libraryfolders" {}"#).unwrap();

        let games = Steam::with_root(&root).installed().await.unwrap();
        std::fs::remove_dir_all(&root).ok();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Hades II");
    }
}
