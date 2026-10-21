//! Steam: installed games from local library manifests. Owned games and
//! playtime (Web API key + SteamID) come later.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use hub_core::{GameEntry, StoreId};
use hub_launch::LaunchAction;

use crate::{vdf, Artwork, LibraryProvider};

/// Not games: runtimes and tools Steam installs alongside them.
const NOT_GAMES: &[&str] = &["228980", "480", "1070560", "1391110", "1628350", "1493710", "1826330"];

pub struct Steam {
    root: Option<PathBuf>,
}

impl Steam {
    /// Locates Steam via the registry, falling back to the default install path.
    pub fn detect() -> Self {
        Self { root: steam_root() }
    }

    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self { root: Some(root.into()) }
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
        Ok(games)
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
