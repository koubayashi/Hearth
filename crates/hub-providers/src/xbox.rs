//! Xbox app and Microsoft Store games, Game Pass included. These are Store
//! packages; the ones built on Microsoft's game kit (nearly every PC Game
//! Pass game) carry a `MicrosoftGame.config` in their folder, which is how a
//! game is told apart from the hundreds of other packages. No sign-in, so
//! only installed games. Older Store games without that file aren't found.
//!
//! Untested against a real install: the development PC has no Xbox games.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use hub_core::{GameEntry, StoreId};
use hub_launch::LaunchAction;

use crate::{Artwork, LibraryProvider, Locked};

/// A Store package as Windows records it for the current user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// `Name_Version_Arch_Resource_PublisherId`.
    pub full_name: String,
    pub root: PathBuf,
    pub display_name: Option<String>,
    /// The package's app ids (usually one, "Game" or "App").
    pub apps: Vec<String>,
}

/// What `MicrosoftGame.config` says about the game.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct GameConfig {
    pub title: Option<String>,
    pub executable_id: Option<String>,
}

/// `Name_PublisherId`, the id that stays the same across updates.
pub fn family_name(full_name: &str) -> Option<String> {
    let parts: Vec<&str> = full_name.split('_').collect();
    match parts.as_slice() {
        [name, .., publisher] if parts.len() >= 3 && !name.is_empty() && !publisher.is_empty() => {
            Some(format!("{name}_{publisher}"))
        }
        _ => None,
    }
}

pub fn parse_config(xml: &str) -> GameConfig {
    GameConfig {
        title: attribute(xml, "ShellVisuals", "DefaultDisplayName").filter(|t| !is_resource(t)),
        executable_id: attribute(xml, "Executable", "Id"),
    }
}

/// The first `name="…"` on the first `<tag …>`. The config is small and
/// machine-written, so a full XML parser isn't needed.
fn attribute(xml: &str, tag: &str, name: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = xml.match_indices(&open).map(|(i, _)| i + open.len()).find(|&i| {
        xml[i..].starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/')
    })?;
    let element = &xml[start..start + xml[start..].find('>')?];
    let key = format!("{name}=\"");
    let mut at = 0;
    while let Some(i) = element[at..].find(&key) {
        let i = at + i;
        // A whole attribute name, not the end of a longer one.
        if element[..i].ends_with(|c: char| c.is_whitespace()) {
            let value = &element[i + key.len()..];
            let value = &value[..value.find('"')?];
            return Some(unescape(value)).filter(|v| !v.trim().is_empty());
        }
        at = i + key.len();
    }
    None
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// A localized string's reference ("ms-resource:…"), not the text itself.
fn is_resource(s: &str) -> bool {
    s.starts_with("ms-resource:")
}

/// The name to show: the game's own, else the Store's, else its folder in
/// `XboxGames` (`…\XboxGames\Forza Horizon 5\Content`), else the package name.
pub fn title(package: &Package, config: &GameConfig) -> String {
    config
        .title
        .clone()
        .or_else(|| package.display_name.clone().filter(|n| !is_resource(n)))
        .or_else(|| {
            let root = &package.root;
            let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
            if name(root).is_some_and(|n| n.eq_ignore_ascii_case("Content")) {
                root.parent().and_then(name)
            } else {
                None
            }
        })
        .unwrap_or_else(|| package.full_name.split('_').next().unwrap_or(&package.full_name).to_string())
}

/// The app to start: the one Windows registered, else the config's.
fn app_id(package: &Package, config: &GameConfig) -> String {
    package.apps.first().cloned().or_else(|| config.executable_id.clone()).unwrap_or_else(|| "App".into())
}

/// Installed games from this PC, plus, once signed in, the games the Xbox
/// account has played with their playtime and art.
pub struct Xbox {
    pub live: std::sync::Arc<crate::xbox_live::XboxLive>,
    art: std::sync::Mutex<std::collections::HashMap<String, crate::Artwork>>,
    descriptions: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl Xbox {
    pub fn new(live: std::sync::Arc<crate::xbox_live::XboxLive>) -> Self {
        Self { live, art: Default::default(), descriptions: Default::default() }
    }
}

struct Game {
    family: String,
    title: String,
    root: PathBuf,
    app: String,
}

fn games(only: Option<&str>) -> Vec<Game> {
    packages(only)
        .into_iter()
        .filter_map(|p| {
            let xml = std::fs::read_to_string(p.root.join("MicrosoftGame.config")).ok()?;
            let config = parse_config(&xml);
            Some(Game { family: family_name(&p.full_name)?, title: title(&p, &config), app: app_id(&p, &config), root: p.root })
        })
        .collect()
}

#[async_trait]
impl LibraryProvider for Xbox {
    fn id(&self) -> StoreId {
        StoreId::Xbox
    }

    async fn installed(&self) -> anyhow::Result<Vec<GameEntry>> {
        Ok(games(None)
            .into_iter()
            .map(|g| GameEntry {
                store: StoreId::Xbox,
                store_game_id: g.family,
                title: g.title,
                installed: true,
                install_dir: Some(g.root.to_string_lossy().into_owned()),
                store_playtime_secs: None,
                store_last_played: None,
            })
            .collect())
    }

    async fn owned(&self) -> anyhow::Result<Vec<GameEntry>> {
        if !self.live.signed_in() {
            return Ok(Vec::new());
        }
        let (titles, playtime) = self.live.library().await?;
        let mut art = self.art.locked();
        let mut descriptions = self.descriptions.locked();
        Ok(titles
            .into_iter()
            .map(|t| {
                art.insert(t.pfn.clone(), Artwork { cover: t.cover, hero: t.hero, logo: t.logo });
                if let Some(d) = t.description {
                    descriptions.insert(t.pfn.clone(), d);
                }
                GameEntry {
                    store: StoreId::Xbox,
                    store_playtime_secs: playtime.get(&t.title_id).copied(),
                    store_last_played: t.last_played,
                    store_game_id: t.pfn,
                    title: t.name,
                    installed: false,
                    install_dir: None,
                }
            })
            .collect())
    }

    fn artwork(&self, store_game_id: &str) -> Artwork {
        self.art.locked().get(store_game_id).cloned().unwrap_or_default()
    }

    fn description(&self, store_game_id: &str) -> Option<String> {
        self.descriptions.locked().get(store_game_id).cloned()
    }

    /// Store apps start through the shell, as the Start menu starts them.
    fn launch(&self, store_game_id: &str) -> LaunchAction {
        match games(Some(store_game_id)).into_iter().next() {
            Some(g) => LaunchAction::Uri { uri: format!(r"shell:AppsFolder\{}!{}", g.family, g.app) },
            None => LaunchAction::Uri { uri: "msxbox://".into() },
        }
    }

    /// The game's Store page, which installs it (Game Pass games included).
    fn install(&self, store_game_id: &str) -> Option<LaunchAction> {
        Some(LaunchAction::Uri { uri: format!("ms-windows-store://pdp/?PFN={store_game_id}") })
    }

    /// Store apps are removed from Windows' installed apps list.
    fn uninstall(&self, _store_game_id: &str) -> Option<LaunchAction> {
        Some(LaunchAction::Uri { uri: "ms-settings:appsfeatures".into() })
    }
}

#[cfg(windows)]
/// Every package, or only the ones of one family (to launch one game without
/// opening all of them).
fn packages(family: Option<&str>) -> Vec<Package> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    const PACKAGES: &str =
        r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";
    let Ok(all) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(PACKAGES) else { return Vec::new() };
    all.enum_keys()
        .flatten()
        .filter(|full_name| family.is_none_or(|f| family_name(full_name).as_deref() == Some(f)))
        .filter_map(|full_name| {
            let key = all.open_subkey(&full_name).ok()?;
            let root: String = key.get_value("PackageRootFolder").ok()?;
            Some(Package {
                root: PathBuf::from(root),
                display_name: key.get_value("DisplayName").ok(),
                apps: key.enum_keys().flatten().collect(),
                full_name,
            })
        })
        .collect()
}

#[cfg(not(windows))]
fn packages(_family: Option<&str>) -> Vec<Package> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from a Game Pass title's MicrosoftGame.config.
    const CONFIG: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Game configVersion="1">
  <Identity Name="Microsoft.624F8B84B80" Publisher="CN=A4954634-DF4B-47C7-AB70-D3215D246AF1" Version="3.643.586.0" />
  <ExecutableList>
    <Executable Name="ForzaHorizon5.exe" TargetDeviceFamily="PC" Id="Game" />
  </ExecutableList>
  <ShellVisuals DefaultDisplayName="Forza Horizon 5" PublisherDisplayName="Xbox Game Studios"
    StoreLogo="StoreLogo.png" Square150x150Logo="Logo.png" />
</Game>"#;

    fn package(root: &str, display: Option<&str>, apps: &[&str]) -> Package {
        Package {
            full_name: "Microsoft.624F8B84B80_3.643.586.0_x64__8wekyb3d8bbwe".into(),
            root: PathBuf::from(root),
            display_name: display.map(String::from),
            apps: apps.iter().map(|a| a.to_string()).collect(),
        }
    }

    #[test]
    fn family_name_drops_version_and_architecture() {
        assert_eq!(
            family_name("Microsoft.624F8B84B80_3.643.586.0_x64__8wekyb3d8bbwe").as_deref(),
            Some("Microsoft.624F8B84B80_8wekyb3d8bbwe")
        );
        assert_eq!(family_name("nonsense"), None);
    }

    #[test]
    fn reads_the_game_config() {
        let c = parse_config(CONFIG);
        assert_eq!(c.title.as_deref(), Some("Forza Horizon 5"));
        assert_eq!(c.executable_id.as_deref(), Some("Game"));
    }

    #[test]
    fn ignores_attributes_that_only_end_the_same() {
        let c = parse_config(r#"<ShellVisuals NotDefaultDisplayName="x" DefaultDisplayName="Halo &amp; Co" />"#);
        assert_eq!(c.title.as_deref(), Some("Halo & Co"));
        assert_eq!(parse_config(r#"<ShellVisualsExtra DefaultDisplayName="x" />"#).title, None);
    }

    #[test]
    fn title_falls_back_past_resource_strings() {
        let config = parse_config(r#"<ShellVisuals DefaultDisplayName="ms-resource:Title" />"#);
        let in_xbox_games = package(r"C:\XboxGames\Forza Horizon 5\Content", Some("ms-resource:Name"), &[]);
        assert_eq!(title(&in_xbox_games, &config), "Forza Horizon 5");
        let named = package(r"C:\Program Files\WindowsApps\X", Some("Sea of Thieves"), &[]);
        assert_eq!(title(&named, &config), "Sea of Thieves");
        let bare = package(r"C:\Program Files\WindowsApps\X", None, &[]);
        assert_eq!(title(&bare, &config), "Microsoft.624F8B84B80");
    }

    #[test]
    fn starts_the_registered_app() {
        let config = parse_config(CONFIG);
        assert_eq!(app_id(&package("C:/x", None, &["App"]), &config), "App");
        assert_eq!(app_id(&package("C:/x", None, &[]), &config), "Game");
    }
}
