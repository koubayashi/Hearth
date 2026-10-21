//! Game details from the Steam store's public API, for every game in the
//! library. Steam games use their own app id; games from other stores are
//! matched by name, the same approach as Playnite's Universal Steam Metadata.
//! No key or sign-in is needed.

use anyhow::Context;
use hub_core::db::normalize_title;
use serde::{Deserialize, Serialize};

const DETAILS_URL: &str = "https://store.steampowered.com/api/appdetails";
const SEARCH_URL: &str = "https://store.steampowered.com/api/storesearch/";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameDetails {
    /// The Steam app the details came from.
    pub steam_app_id: u64,
    /// As Steam shows it, e.g. "May 31, 2024".
    pub release_date: Option<String>,
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
    pub genres: Vec<String>,
    /// A short, curated list (single-player, co-op, controller, cloud saves…).
    pub features: Vec<String>,
    pub metacritic: Option<u32>,
    pub metacritic_url: Option<String>,
    /// Number of Steam reviews recommending the game.
    pub recommendations: Option<u64>,
}

pub struct Metadata {
    http: reqwest::Client,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            http: reqwest::Client::builder().user_agent("Hearth/0.1").build().expect("http client"),
        }
    }
}

impl Metadata {
    /// Details for a Steam app id.
    pub async fn steam_app(&self, app_id: u64) -> anyhow::Result<Option<GameDetails>> {
        let id = app_id.to_string();
        let body: serde_json::Value = self
            .http
            .get(DETAILS_URL)
            .query(&[("appids", id.as_str()), ("l", "english"), ("cc", "us")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("reading Steam store details")?;
        Ok(parse_details(&body, app_id))
    }

    /// Finds the Steam store page for a game from another store, by title.
    pub async fn find_on_steam(&self, title: &str) -> anyhow::Result<Option<u64>> {
        let body: SearchResponse = self
            .http
            .get(SEARCH_URL)
            .query(&[("term", title), ("l", "english"), ("cc", "us")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("searching the Steam store")?;
        Ok(best_match(title, &body.items))
    }
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    items: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct SearchItem {
    id: u64,
    name: String,
}

/// Only an exact title match (ignoring case, punctuation and ™/®) counts, so a
/// similar-sounding game never lends its details.
fn best_match(title: &str, items: &[SearchItem]) -> Option<u64> {
    let want = normalize_title(title);
    items.iter().find(|i| normalize_title(&i.name) == want).map(|i| i.id)
}

/// Steam categories worth showing, in display order, with friendlier names.
const FEATURES: &[(u32, &str)] = &[
    (2, "Single-player"),
    (1, "Multiplayer"),
    (9, "Co-op"),
    (24, "Split screen"),
    (28, "Full controller support"),
    (18, "Partial controller support"),
    (23, "Cloud saves"),
    (22, "Achievements"),
    (30, "Workshop"),
    (61, "HDR"),
    (53, "VR"),
];

#[derive(Deserialize)]
struct Details {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    developers: Vec<String>,
    #[serde(default)]
    publishers: Vec<String>,
    #[serde(default)]
    genres: Vec<Described>,
    #[serde(default)]
    categories: Vec<Category>,
    metacritic: Option<Metacritic>,
    recommendations: Option<Recommendations>,
    release_date: Option<ReleaseDate>,
}

#[derive(Deserialize)]
struct Described {
    description: String,
}

#[derive(Deserialize)]
struct Category {
    id: u32,
}

#[derive(Deserialize)]
struct Metacritic {
    score: u32,
    url: Option<String>,
}

#[derive(Deserialize)]
struct Recommendations {
    total: u64,
}

#[derive(Deserialize)]
struct ReleaseDate {
    date: Option<String>,
}

fn parse_details(body: &serde_json::Value, app_id: u64) -> Option<GameDetails> {
    let entry = body.get(app_id.to_string())?;
    if entry.get("success")?.as_bool() != Some(true) {
        return None;
    }
    let d: Details = serde_json::from_value(entry.get("data")?.clone()).ok()?;
    if d.kind.as_deref().is_some_and(|k| k != "game") {
        return None;
    }
    let has = |id: u32| d.categories.iter().any(|c| c.id == id);
    let mut features: Vec<String> = FEATURES.iter().filter(|(id, _)| has(*id)).map(|(_, n)| n.to_string()).collect();
    // Full controller support makes "partial" redundant.
    if has(28) {
        features.retain(|f| f != "Partial controller support");
    }
    Some(GameDetails {
        steam_app_id: app_id,
        release_date: d.release_date.and_then(|r| r.date).filter(|s| !s.is_empty()),
        developers: d.developers,
        publishers: d.publishers,
        genres: d.genres.into_iter().map(|g| g.description).collect(),
        features,
        metacritic: d.metacritic.as_ref().map(|m| m.score),
        metacritic_url: d.metacritic.and_then(|m| m.url),
        recommendations: d.recommendations.map(|r| r.total),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_store_details_into_a_short_profile() {
        let body: serde_json::Value = serde_json::from_str(
            r#"{"2488620":{"success":true,"data":{"type":"game","name":"F1® 24",
              "developers":["Codemasters"],"publishers":["Electronic Arts"],
              "genres":[{"id":"9","description":"Racing"},{"id":"18","description":"Sports"}],
              "categories":[{"id":2},{"id":1},{"id":9},{"id":28},{"id":18},{"id":23},{"id":22},{"id":62}],
              "metacritic":{"score":72,"url":"https://www.metacritic.com/game/pc/f1-24"},
              "recommendations":{"total":15643},
              "release_date":{"coming_soon":false,"date":"May 31, 2024"}}}}"#,
        )
        .unwrap();
        let d = parse_details(&body, 2488620).unwrap();
        assert_eq!(d.release_date.as_deref(), Some("May 31, 2024"));
        assert_eq!(d.developers, ["Codemasters"]);
        assert_eq!(d.genres, ["Racing", "Sports"]);
        assert_eq!(d.features, ["Single-player", "Multiplayer", "Co-op", "Full controller support", "Cloud saves", "Achievements"]);
        assert_eq!(d.metacritic, Some(72));
        assert_eq!(d.recommendations, Some(15643));
    }

    #[test]
    fn rejects_failures_and_non_games() {
        let fail: serde_json::Value = serde_json::from_str(r#"{"1":{"success":false}}"#).unwrap();
        assert!(parse_details(&fail, 1).is_none());
        let dlc: serde_json::Value = serde_json::from_str(r#"{"2":{"success":true,"data":{"type":"dlc"}}}"#).unwrap();
        assert!(parse_details(&dlc, 2).is_none());
    }

    #[test]
    fn matches_titles_exactly_not_loosely() {
        let items = vec![
            SearchItem { id: 1174180, name: "Red Dead Redemption 2".into() },
            SearchItem { id: 99, name: "Deadrock Redemption".into() },
        ];
        assert_eq!(best_match("Red Dead Redemption 2", &items), Some(1174180));
        assert_eq!(best_match("RED DEAD REDEMPTION 2™", &items), Some(1174180));
        assert_eq!(best_match("Red Dead Redemption", &items), None);
    }
}
