use serde::{Deserialize, Serialize};

/// A storefront or source a game can come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoreId {
    Steam,
    Epic,
    Xbox,
    Gog,
    Ea,
    Ubisoft,
    Manual,
}

impl StoreId {
    pub const ALL: [StoreId; 7] = [
        StoreId::Steam,
        StoreId::Epic,
        StoreId::Xbox,
        StoreId::Gog,
        StoreId::Ea,
        StoreId::Ubisoft,
        StoreId::Manual,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            StoreId::Steam => "steam",
            StoreId::Epic => "epic",
            StoreId::Xbox => "xbox",
            StoreId::Gog => "gog",
            StoreId::Ea => "ea",
            StoreId::Ubisoft => "ubisoft",
            StoreId::Manual => "manual",
        }
    }

    pub fn parse(s: &str) -> Option<StoreId> {
        StoreId::ALL.into_iter().find(|id| id.as_str() == s)
    }
}

/// What a provider reports about one game in one store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameEntry {
    pub store: StoreId,
    pub store_game_id: String,
    pub title: String,
    pub installed: bool,
    pub install_dir: Option<String>,
    /// Total playtime the store reports, in seconds, if it reports any.
    pub store_playtime_secs: Option<i64>,
    /// Unix seconds.
    pub store_last_played: Option<i64>,
}

/// A store entry as persisted, linked to a (possibly merged) game.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoreEntry {
    pub id: i64,
    pub game_id: i64,
    pub store: StoreId,
    pub store_game_id: String,
    pub installed: bool,
    pub install_dir: Option<String>,
    pub hidden: bool,
    pub store_playtime_secs: Option<i64>,
    pub store_baseline_secs: Option<i64>,
}

/// A game as shown in the library: one tile, possibly backed by several stores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Game {
    pub id: i64,
    pub title: String,
    pub favorite: bool,
    pub hidden: bool,
    pub installed: bool,
    pub added_at: i64,
    pub last_played: Option<i64>,
    pub playtime_secs: i64,
    pub cover: Option<String>,
    pub hero: Option<String>,
    pub logo: Option<String>,
    pub description: Option<String>,
    pub entries: Vec<StoreEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionSource {
    /// Launched from Hearth.
    Hub,
    /// Detected by the tray tracker.
    Tracker,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaySession {
    pub id: i64,
    pub game_id: i64,
    pub started_at: i64,
    pub ended_at: i64,
    pub source: SessionSource,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallFilter {
    #[default]
    All,
    Installed,
    NotInstalled,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryQuery {
    pub install: InstallFilter,
    /// Empty means every store.
    pub stores: Vec<StoreId>,
    pub search: Option<String>,
    /// When true, return only hidden games (the "Hidden" view).
    pub hidden_only: bool,
}
