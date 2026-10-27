//! Game details from the Steam store's public API, for every game in the
//! library. Steam games use their own app id; games from other stores are
//! matched by name, the same approach as Playnite's Universal Steam Metadata.
//! No key or sign-in is needed.

use anyhow::Context;
use hub_core::db::normalize_title;
use serde::{Deserialize, Serialize};

const DETAILS_URL: &str = "https://store.steampowered.com/api/appdetails";
const SEARCH_URL: &str = "https://store.steampowered.com/api/storesearch/";
const ASSETS_URL: &str = "https://api.steampowered.com/IStoreBrowseService/GetItems/v1/";

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
    /// Steam's one-paragraph description, as plain text. Missing from
    /// details cached before it was kept (they are fetched again).
    #[serde(default)]
    pub summary: Option<String>,
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

    /// The real artwork addresses for Steam apps. Steam moved most artwork to
    /// content-hashed paths, so guessed URLs 404 for newer or updated games.
    pub async fn steam_art(&self, app_ids: &[u64]) -> anyhow::Result<std::collections::HashMap<u64, crate::Artwork>> {
        let mut out = std::collections::HashMap::new();
        for chunk in app_ids.chunks(50) {
            let ids: Vec<serde_json::Value> = chunk.iter().map(|id| serde_json::json!({ "appid": id })).collect();
            let input = serde_json::json!({
                "ids": ids,
                "context": { "language": "english", "country_code": "US" },
                "data_request": { "include_assets": true },
            });
            let body: serde_json::Value = self
                .http
                .get(ASSETS_URL)
                .query(&[("input_json", input.to_string())])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await
                .context("reading Steam artwork")?;
            out.extend(parse_assets(&body));
        }
        Ok(out)
    }

    /// Finds the Steam store page for a game from another store, by title.
    /// If the plain title finds nothing, the common edition names are tried,
    /// since Steam often lists older games as e.g. "… Director's Cut Edition".
    pub async fn find_on_steam(&self, title: &str) -> anyhow::Result<Option<u64>> {
        let items = self.search(title).await?;
        if let Some(id) = best_match(title, &items) {
            return Ok(Some(id));
        }
        for suffix in ["Director's Cut", "Game of the Year", "Definitive Edition", "Remastered"] {
            let more = self.search(&format!("{title} {suffix}")).await?;
            if let Some(id) = best_match(title, &more) {
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    /// Steam store search results: (app id, name).
    pub async fn search_store(&self, term: &str) -> anyhow::Result<Vec<(u64, String)>> {
        Ok(self.search(term).await?.into_iter().map(|i| (i.id, i.name)).collect())
    }

    async fn search(&self, term: &str) -> anyhow::Result<Vec<SearchItem>> {
        let body: SearchResponse = self
            .http
            .get(SEARCH_URL)
            .query(&[("term", term), ("l", "english"), ("cc", "us")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("searching the Steam store")?;
        Ok(body.items)
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

/// Only the same game counts: titles must match exactly once case,
/// punctuation, ™/® and edition names are set aside. "Assassin's Creed"
/// matches "Assassin's Creed™: Director's Cut Edition" but never
/// "Assassin's Creed II", so a similar-sounding game never lends its details.
fn best_match(title: &str, items: &[SearchItem]) -> Option<u64> {
    let want = base_title(title);
    if want.is_empty() {
        return None;
    }
    items.iter().find(|i| base_title(&i.name) == want).map(|i| i.id)
}

/// Words that name an edition rather than a different game.
const EDITION_WORDS: &[&str] = &[
    "director", "directors", "cut", "edition", "goty", "game", "of", "the", "year", "definitive",
    "complete", "enhanced", "remastered", "deluxe", "gold", "ultimate", "standard", "special",
    "collectors", "anniversary", "hd", "classic", "premium", "premier",
];

pub fn base_title(title: &str) -> String {
    let lower = title.to_lowercase().replace(['’', '\''], "");
    // "Name: Something Edition" or "Name - Something Edition".
    let main = match lower.find(':').or_else(|| lower.find(" - ")) {
        Some(i) if is_edition(&lower[i..]) => &lower[..i],
        _ => lower.as_str(),
    };
    // Trailing edition words without a separator: "Name Definitive Edition".
    let mut words: Vec<&str> = main.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    if words.len() > 1 {
        if let Some(cut) = words.iter().rposition(|w| !EDITION_WORDS.contains(w)) {
            let tail = &words[cut + 1..];
            if tail.iter().any(|w| ["edition", "cut", "goty", "remastered", "definitive", "enhanced"].contains(w)) {
                words.truncate(cut + 1);
            }
        }
    }
    normalize_title(&words.join(" "))
}

fn is_edition(rest: &str) -> bool {
    let words: Vec<&str> = rest.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    !words.is_empty() && words.iter().all(|w| EDITION_WORDS.contains(w))
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
    short_description: Option<String>,
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

/// Steam's short descriptions are HTML: tags dropped, entities decoded,
/// spaces collapsed.
fn plain_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                text.push(' ');
            }
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    let text = text
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
        summary: d.short_description.as_deref().map(plain_text).filter(|s| !s.is_empty()),
    })
}

const ASSET_BASE: &str = "https://shared.steamstatic.com/store_item_assets/";

fn parse_assets(body: &serde_json::Value) -> std::collections::HashMap<u64, crate::Artwork> {
    let mut out = std::collections::HashMap::new();
    let Some(items) = body.pointer("/response/store_items").and_then(|v| v.as_array()) else { return out };
    for item in items {
        let (Some(id), Some(assets)) = (item.get("appid").and_then(|v| v.as_u64()), item.get("assets")) else {
            continue;
        };
        let Some(format) = assets.get("asset_url_format").and_then(|v| v.as_str()) else { continue };
        let url = |key: &str| {
            assets
                .get(key)
                .and_then(|v| v.as_str())
                .map(|file| format!("{ASSET_BASE}{}", format.replace("${FILENAME}", file)))
        };
        out.insert(
            id,
            crate::Artwork {
                cover: url("library_capsule"),
                hero: url("library_hero").or_else(|| url("hero_capsule")),
                logo: url("library_logo"),
            },
        );
    }
    out
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
              "release_date":{"coming_soon":false,"date":"May 31, 2024"},
              "short_description":"Be the last piece of the puzzle in &quot;F1® 24&quot;.<br> Race &amp; win."}}}"#,
        )
        .unwrap();
        let d = parse_details(&body, 2488620).unwrap();
        assert_eq!(d.release_date.as_deref(), Some("May 31, 2024"));
        assert_eq!(d.developers, ["Codemasters"]);
        assert_eq!(d.genres, ["Racing", "Sports"]);
        assert_eq!(d.features, ["Single-player", "Multiplayer", "Co-op", "Full controller support", "Cloud saves", "Achievements"]);
        assert_eq!(d.metacritic, Some(72));
        assert_eq!(d.recommendations, Some(15643));
        assert_eq!(d.summary.as_deref(), Some("Be the last piece of the puzzle in \"F1® 24\". Race & win."));
    }

    #[test]
    fn builds_hashed_artwork_urls() {
        let body: serde_json::Value = serde_json::from_str(r#"{"response":{"store_items":[{"appid":3639070,"assets":{
            "asset_url_format":"steam/apps/3639070/${FILENAME}?t=1790597941",
            "library_capsule":"412f/library_capsule.jpg","library_hero":"2221/library_hero.jpg"}}]}}"#).unwrap();
        let art = &parse_assets(&body)[&3639070];
        assert_eq!(art.cover.as_deref(), Some("https://shared.steamstatic.com/store_item_assets/steam/apps/3639070/412f/library_capsule.jpg?t=1790597941"));
        assert_eq!(art.hero.as_deref(), Some("https://shared.steamstatic.com/store_item_assets/steam/apps/3639070/2221/library_hero.jpg?t=1790597941"));
        assert_eq!(art.logo, None);
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

    #[test]
    fn editions_match_but_sequels_do_not() {
        let items = vec![
            SearchItem { id: 33230, name: "Assassin's Creed 2".into() },
            SearchItem { id: 15100, name: "Assassin's Creed™: Director's Cut Edition".into() },
        ];
        assert_eq!(best_match("Assassin's Creed", &items), Some(15100));
        assert_eq!(best_match("Assassin's Creed II", &items[..1]), None);
        let more = vec![
            SearchItem { id: 1, name: "Death Stranding Director's Cut".into() },
            SearchItem { id: 2, name: "The Witcher 3: Wild Hunt".into() },
        ];
        assert_eq!(best_match("DEATH STRANDING", &more), Some(1));
        // A subtitle that isn't an edition is part of the name.
        assert_eq!(best_match("The Witcher 3", &more), None);
    }
}
