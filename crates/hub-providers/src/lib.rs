//! Store integrations. Each store implements [`LibraryProvider`]; the app
//! syncs every enabled provider into the local database.

use async_trait::async_trait;
use hub_core::{GameEntry, StoreId};
use hub_launch::LaunchAction;

#[async_trait]
pub trait LibraryProvider: Send + Sync {
    fn id(&self) -> StoreId;

    /// Games installed on this PC. Must work without a store login.
    async fn installed(&self) -> anyhow::Result<Vec<GameEntry>>;

    /// Every game on the account, installed or not. Empty when logged out.
    async fn owned(&self) -> anyhow::Result<Vec<GameEntry>> {
        Ok(Vec::new())
    }

    fn launch(&self, store_game_id: &str) -> LaunchAction;

    fn install(&self, _store_game_id: &str) -> Option<LaunchAction> {
        None
    }

    fn uninstall(&self, _store_game_id: &str) -> Option<LaunchAction> {
        None
    }
}
