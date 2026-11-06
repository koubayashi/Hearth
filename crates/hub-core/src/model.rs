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
    /// Folder games: the program Play starts instead of the scan's choice,
    /// relative to `install_dir`.
    #[serde(default)]
    pub launch_exe: Option<String>,
    /// Its install folder is on a drive that isn't connected right now.
    #[serde(default)]
    pub offline: bool,
    /// The store's own name for it ("Grand Theft Auto V Enhanced"): a
    /// version's name once games are combined.
    #[serde(default)]
    pub title: Option<String>,
    /// When the store last saw it played, if it says.
    #[serde(default)]
    pub last_played: Option<i64>,
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
    /// Where the player is with it; None means not set.
    pub status: Option<GameStatus>,
    /// Ids of the user's collections this game is in.
    pub collections: Vec<i64>,
    /// Set when the game's only install is on a drive that isn't connected
    /// (e.g. "E:"): it can't be played until that drive is back.
    #[serde(default)]
    pub offline_drive: Option<String>,
    /// Ways to play the user added (a script, a mod loader, another exe).
    #[serde(default)]
    pub launch_options: Vec<LaunchOption>,
}

/// A way to play a game besides its store's own launch, e.g. a server
/// replacement's start script. Runs the program the user picked, nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchOption {
    pub id: i64,
    pub game_id: i64,
    pub name: String,
    pub path: String,
    /// Passed exactly as typed, like a shortcut's arguments.
    pub args: String,
    pub working_dir: Option<String>,
    /// The Play button uses this one instead of the store.
    pub is_default: bool,
    /// Its program isn't on disk any more (a deleted game folder took it).
    /// A bare program name ("explorer.exe") is found on PATH, so it never is.
    #[serde(default)]
    pub missing: bool,
}

/// Backlog-style progress, set by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GameStatus {
    Playing,
    Backlog,
    Completed,
    Abandoned,
}

impl GameStatus {
    pub const ALL: [GameStatus; 4] = [GameStatus::Playing, GameStatus::Backlog, GameStatus::Completed, GameStatus::Abandoned];

    pub fn as_str(self) -> &'static str {
        match self {
            GameStatus::Playing => "playing",
            GameStatus::Backlog => "backlog",
            GameStatus::Completed => "completed",
            GameStatus::Abandoned => "abandoned",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|x| x.as_str() == s)
    }
}

/// A user-made list of games.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub count: i64,
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
#[serde(default)]
pub struct LibraryQuery {
    pub install: InstallFilter,
    /// Empty means every store.
    pub stores: Vec<StoreId>,
    pub search: Option<String>,
    /// When true, return only hidden games (the "Hidden" view).
    pub hidden_only: bool,
    /// Only games with this status.
    pub status: Option<GameStatus>,
    /// Only games in this collection.
    pub collection: Option<i64>,
}

/// A series, franchise or company games can be grouped by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub label: String,
}

/// `child` is part of `parent` (a studio of a publisher, a series of a franchise).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityParent {
    pub child: String,
    pub parent: String,
}

/// A game is in a series, or was made or published by a company.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameLink {
    pub game_id: i64,
    /// "series", "developer" or "publisher".
    pub role: String,
    pub entity: String,
    /// Added by the user rather than found on Wikidata.
    pub by_user: bool,
}

/// Everything the "browse by" shelves need, in one read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartIndex {
    pub entities: Vec<Entity>,
    pub parents: Vec<EntityParent>,
    pub links: Vec<GameLink>,
}
