//! Store integrations. Each store implements [`LibraryProvider`]; the app
//! syncs every enabled provider into the local database.

pub mod epic;
pub mod secrets;
pub mod steam;
pub mod vdf;

use async_trait::async_trait;
use hub_core::{GameEntry, StoreId};
use hub_launch::LaunchAction;

/// Artwork URLs a store can supply directly, before the metadata service exists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Artwork {
    pub cover: Option<String>,
    pub hero: Option<String>,
    pub logo: Option<String>,
}

#[async_trait]
pub trait LibraryProvider: Send + Sync {
    fn id(&self) -> StoreId;

    /// Games installed on this PC. Must work without a store login.
    async fn installed(&self) -> anyhow::Result<Vec<GameEntry>>;

    /// Every game on the account, installed or not. Empty when logged out.
    async fn owned(&self) -> anyhow::Result<Vec<GameEntry>> {
        Ok(Vec::new())
    }

    /// Store ids that are known not to be games (runtimes, tools); removed on sync.
    fn excluded_ids(&self) -> &[&str] {
        &[]
    }

    fn artwork(&self, _store_game_id: &str) -> Artwork {
        Artwork::default()
    }

    fn launch(&self, store_game_id: &str) -> LaunchAction;

    fn install(&self, _store_game_id: &str) -> Option<LaunchAction> {
        None
    }

    fn uninstall(&self, _store_game_id: &str) -> Option<LaunchAction> {
        None
    }
}

/// Everything a store knows: owned games, with install state and folders taken
/// from the local scan. Installed games missing from the owned list (logged
/// out, or not yet synced) are kept too.
pub fn merge_entries(installed: Vec<GameEntry>, owned: Vec<GameEntry>) -> Vec<GameEntry> {
    let mut by_id: std::collections::HashMap<String, GameEntry> =
        owned.into_iter().map(|g| (g.store_game_id.clone(), g)).collect();
    for local in installed {
        match by_id.get_mut(&local.store_game_id) {
            Some(g) => {
                g.installed = local.installed;
                g.install_dir = local.install_dir;
                g.store_last_played = g.store_last_played.or(local.store_last_played);
            }
            None => {
                by_id.insert(local.store_game_id.clone(), local);
            }
        }
    }
    let mut out: Vec<GameEntry> = by_id.into_values().collect();
    out.sort_by(|a, b| a.store_game_id.cmp(&b.store_game_id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: &str, installed: bool, playtime: Option<i64>) -> GameEntry {
        GameEntry {
            store: StoreId::Epic,
            store_game_id: id.into(),
            title: id.into(),
            installed,
            install_dir: installed.then(|| format!("D:/{id}")),
            store_playtime_secs: playtime,
            store_last_played: None,
        }
    }

    #[test]
    fn merge_keeps_owned_playtime_and_local_install_state() {
        let merged = merge_entries(vec![e("a", true, None), e("local-only", true, None)], vec![e("a", false, Some(60)), e("b", false, None)]);
        let ids: Vec<_> = merged.iter().map(|g| (g.store_game_id.as_str(), g.installed, g.store_playtime_secs)).collect();
        assert_eq!(ids, [("a", true, Some(60)), ("b", false, None), ("local-only", true, None)]);
        assert_eq!(merged[0].install_dir.as_deref(), Some("D:/a"));
    }
}
