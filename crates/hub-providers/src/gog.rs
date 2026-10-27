//! GOG: games installed by GOG Galaxy or by GOG's offline installers, which
//! both record each game in the registry; signed in, the account's other games
//! come from [`crate::gog_web`]. Playtime isn't available.
//!
//! Untested against a real install: the format follows GOG's installers and
//! the one stale entry on the development PC.

use std::path::PathBuf;

use async_trait::async_trait;
use hub_core::{GameEntry, StoreId};
use hub_launch::LaunchAction;

use crate::{Artwork, LibraryProvider};

/// One game as GOG's installer recorded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GogInstall {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub exe: PathBuf,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub uninstaller: Option<PathBuf>,
}

impl GogInstall {
    /// From the values under `GOG.com\Games\<id>`. DLC (which depends on its
    /// game) is left out, and so is an entry whose folder is gone: deleting
    /// the folder by hand leaves the registry entry behind.
    pub fn from_values(get: impl Fn(&str) -> Option<String>) -> Option<Self> {
        let value = |name: &str| get(name).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        if value("dependsOn").is_some() {
            return None;
        }
        let id = value("gameID").or_else(|| value("productID"))?;
        let path = PathBuf::from(value("path")?);
        let exe = PathBuf::from(value("exe")?);
        Some(Self {
            title: value("gameName").or_else(|| value("startMenu")).unwrap_or_else(|| id.clone()),
            args: value("launchParam").map(|a| a.split_whitespace().map(String::from).collect()).unwrap_or_default(),
            working_dir: value("workingDir").map(PathBuf::from),
            uninstaller: value("uninstallCommand").map(PathBuf::from),
            id,
            path,
            exe,
        })
    }
}

pub struct Gog {
    /// GalaxyClient.exe, when GOG Galaxy is installed.
    galaxy: Option<PathBuf>,
    /// The GOG account, for games owned but not installed.
    pub web: std::sync::Arc<crate::gog_web::GogWeb>,
}

impl Gog {
    pub fn detect(web: std::sync::Arc<crate::gog_web::GogWeb>) -> Self {
        Self { galaxy: galaxy_exe(), web }
    }

    fn find(&self, id: &str) -> Option<GogInstall> {
        installs().into_iter().find(|g| g.id == id)
    }
}

#[async_trait]
impl LibraryProvider for Gog {
    fn id(&self) -> StoreId {
        StoreId::Gog
    }

    async fn installed(&self) -> anyhow::Result<Vec<GameEntry>> {
        Ok(installs()
            .into_iter()
            .filter(|g| g.path.is_dir())
            .map(|g| GameEntry {
                store: StoreId::Gog,
                store_game_id: g.id,
                title: g.title,
                installed: true,
                install_dir: Some(g.path.to_string_lossy().into_owned()),
                store_playtime_secs: None,
                store_last_played: None,
            })
            .collect())
    }

    /// Through Galaxy when it's installed (cloud saves, its overlay, its own
    /// playtime), else the game's own program, as GOG's shortcut would.
    fn launch(&self, store_game_id: &str) -> LaunchAction {
        let game = self.find(store_game_id);
        match (&self.galaxy, game) {
            (Some(galaxy), Some(g)) => LaunchAction::Process {
                exe: galaxy.clone(),
                args: vec!["/command=runGame".into(), format!("/gameId={}", g.id), format!("/path={}", g.path.display())],
                working_dir: galaxy.parent().map(PathBuf::from),
            },
            (None, Some(g)) => LaunchAction::Process { exe: g.exe, args: g.args, working_dir: g.working_dir },
            (_, None) => LaunchAction::Uri { uri: format!("goggalaxy://openGameView/{store_game_id}") },
        }
    }

    fn uninstall(&self, store_game_id: &str) -> Option<LaunchAction> {
        let exe = self.find(store_game_id)?.uninstaller.filter(|u| u.is_file())?;
        Some(LaunchAction::Process { exe, args: Vec::new(), working_dir: None })
    }

    /// Every game on the account once signed in; DLC is left out.
    async fn owned(&self) -> anyhow::Result<Vec<GameEntry>> {
        if !self.web.signed_in() {
            return Ok(Vec::new());
        }
        let ids = self.web.owned_ids().await?;
        self.web.describe(&ids).await;
        Ok(ids
            .into_iter()
            .filter_map(|id| {
                let p = self.web.product(&id).filter(|p| p.is_game)?;
                Some(GameEntry {
                    store: StoreId::Gog,
                    store_game_id: id,
                    title: p.title,
                    installed: false,
                    install_dir: None,
                    store_playtime_secs: None,
                    store_last_played: None,
                })
            })
            .collect())
    }

    fn artwork(&self, store_game_id: &str) -> Artwork {
        self.web
            .product(store_game_id)
            .map(|p| Artwork { cover: p.cover, hero: p.hero, logo: p.logo })
            .unwrap_or_default()
    }

    fn description(&self, store_game_id: &str) -> Option<String> {
        self.web.product(store_game_id)?.description
    }

    /// Galaxy's page for the game, which installs it; without Galaxy, the
    /// GOG account's games page, where the installers are.
    fn install(&self, store_game_id: &str) -> Option<LaunchAction> {
        let uri = match self.galaxy {
            Some(_) => format!("goggalaxy://openGameView/{store_game_id}"),
            None => "https://www.gog.com/account".into(),
        };
        Some(LaunchAction::Uri { uri })
    }
}

#[cfg(windows)]
fn installs() -> Vec<GogInstall> {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
    let Ok(games) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\WOW6432Node\GOG.com\Games") else {
        return Vec::new();
    };
    games
        .enum_keys()
        .flatten()
        .filter_map(|name| games.open_subkey(name).ok())
        .filter_map(|key| GogInstall::from_values(|v| key.get_value::<String, _>(v).ok()))
        .collect()
}

#[cfg(not(windows))]
fn installs() -> Vec<GogInstall> {
    Vec::new()
}

#[cfg(windows)]
fn galaxy_exe() -> Option<PathBuf> {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
    let key = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\WOW6432Node\GOG.com\GalaxyClient").ok()?;
    let dir: String = key.open_subkey("paths").ok()?.get_value("client").ok()?;
    let exe: String = key.get_value("clientExecutable").unwrap_or_else(|_| "GalaxyClient.exe".into());
    Some(PathBuf::from(dir).join(exe)).filter(|p| p.is_file())
}

#[cfg(not(windows))]
fn galaxy_exe() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn entry(values: &[(&str, &str)]) -> Option<GogInstall> {
        let map: HashMap<&str, &str> = values.iter().copied().collect();
        GogInstall::from_values(|k| map.get(k).map(|v| v.to_string()))
    }

    // As GOG's offline installer wrote it on the development PC.
    const PCBS: &[(&str, &str)] = &[
        ("gameName", "PC Building Simulator"),
        ("gameID", "2147483071"),
        ("productID", "2147483071"),
        ("path", r"D:\Games\PC Building Simulator 2\PC Building Simulator"),
        ("exe", r"D:\Games\PC Building Simulator 2\PC Building Simulator\PCBS.exe"),
        ("launchParam", ""),
        ("workingDir", r"D:\Games\PC Building Simulator 2\PC Building Simulator"),
        ("uninstallCommand", r"D:\Games\PC Building Simulator 2\PC Building Simulator\unins000.exe"),
        ("dependsOn", ""),
        ("startMenu", "PC Building Simulator"),
    ];

    #[test]
    fn reads_an_installer_entry() {
        let g = entry(PCBS).unwrap();
        assert_eq!(g.id, "2147483071");
        assert_eq!(g.title, "PC Building Simulator");
        assert_eq!(g.exe, PathBuf::from(r"D:\Games\PC Building Simulator 2\PC Building Simulator\PCBS.exe"));
        assert!(g.args.is_empty());
    }

    #[test]
    fn leaves_out_dlc() {
        let mut dlc = PCBS.to_vec();
        dlc.retain(|(k, _)| *k != "dependsOn");
        dlc.push(("dependsOn", "1207658930"));
        assert_eq!(entry(&dlc), None);
    }

    #[test]
    fn splits_launch_arguments() {
        let mut v = PCBS.to_vec();
        v.retain(|(k, _)| *k != "launchParam");
        v.push(("launchParam", "-windowed  -skipintro"));
        assert_eq!(entry(&v).unwrap().args, ["-windowed", "-skipintro"]);
    }
}
