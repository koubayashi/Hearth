//! SteamGridDB: community-made covers, backgrounds and logos. Needs the
//! user's own free API key (steamgriddb.com → Preferences → API).

use std::sync::Mutex;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

const API: &str = "https://www.steamgriddb.com/api/v2";
pub const KEY_PAGE: &str = "https://www.steamgriddb.com/profile/preferences/api";

/// What a picture is for in Hearth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtKind {
    /// Tall 2:3 library cover ("grid" in SteamGridDB terms).
    Cover,
    /// Wide background ("hero").
    Hero,
    Logo,
}

impl ArtKind {
    fn endpoint(self) -> &'static str {
        match self {
            ArtKind::Cover => "grids",
            ArtKind::Hero => "heroes",
            ArtKind::Logo => "logos",
        }
    }

    fn filters(self) -> &'static [(&'static str, &'static str)] {
        // Still images only, nothing NSFW, joke art or flashing.
        const SAFE: [(&str, &str); 3] = [("nsfw", "false"), ("humor", "false"), ("epilepsy", "false")];
        match self {
            ArtKind::Cover => &[("dimensions", "600x900"), ("types", "static"), SAFE[0], SAFE[1], SAFE[2]],
            ArtKind::Hero => &[("types", "static"), SAFE[0], SAFE[1], SAFE[2]],
            ArtKind::Logo => &[("types", "static"), SAFE[0], SAFE[1], SAFE[2]],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtOption {
    pub url: String,
    pub thumb: String,
    pub score: i64,
    pub style: String,
}

/// Where to look a game up: its Steam app, or a SteamGridDB game id.
#[derive(Debug, Clone, Copy)]
pub enum GameRef {
    Steam(u64),
    Sgdb(u64),
}

pub struct SteamGridDb {
    http: reqwest::Client,
    key: Mutex<Option<String>>,
}

impl SteamGridDb {
    pub fn new(key: Option<String>) -> Self {
        Self {
            http: reqwest::Client::builder().user_agent("Hearth/0.1").build().expect("http client"),
            key: Mutex::new(key.filter(|k| !k.is_empty())),
        }
    }

    pub fn has_key(&self) -> bool {
        self.key.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }

    pub fn set_key(&self, key: Option<String>) {
        *self.key.lock().unwrap_or_else(|e| e.into_inner()) = key;
    }

    fn key(&self) -> anyhow::Result<String> {
        self.key
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .context("Add a SteamGridDB API key in Settings first")
    }

    async fn get(&self, key: &str, path: &str, query: &[(&str, &str)]) -> anyhow::Result<Option<serde_json::Value>> {
        let res = self.http.get(format!("{API}{path}")).bearer_auth(key).query(query).send().await?;
        match res.status().as_u16() {
            200 => {}
            // Unknown game or app: not an error, just nothing there.
            404 => return Ok(None),
            401 | 403 => bail!("SteamGridDB rejected the API key. Check it at {KEY_PAGE}."),
            429 => bail!("SteamGridDB is rate limiting Hearth; it will try again later."),
            code => bail!("SteamGridDB returned {code}."),
        }
        let body: Envelope = res.json().await.context("reading SteamGridDB")?;
        Ok(body.success.then_some(body.data))
    }

    /// Validates a key before it's saved.
    pub async fn check_key(&self, key: &str) -> anyhow::Result<()> {
        self.get(key, "/search/autocomplete/portal", &[]).await?;
        Ok(())
    }

    /// Finds a game by title, matching exactly as the Steam lookup does.
    pub async fn find_game(&self, title: &str) -> anyhow::Result<Option<u64>> {
        let key = self.key()?;
        let path = format!("/search/autocomplete/{}", urlencode(title));
        let Some(data) = self.get(&key, &path, &[]).await? else { return Ok(None) };
        let games: Vec<SearchGame> = serde_json::from_value(data).unwrap_or_default();
        let want = crate::metadata::base_title(title);
        Ok(games.into_iter().find(|g| crate::metadata::base_title(&g.name) == want).map(|g| g.id))
    }

    /// Images for a game, best first (SteamGridDB orders by community score).
    pub async fn images(&self, game: GameRef, kind: ArtKind) -> anyhow::Result<Vec<ArtOption>> {
        let key = self.key()?;
        let path = match game {
            GameRef::Steam(id) => format!("/{}/steam/{id}", kind.endpoint()),
            GameRef::Sgdb(id) => format!("/{}/game/{id}", kind.endpoint()),
        };
        let Some(data) = self.get(&key, &path, kind.filters()).await? else { return Ok(Vec::new()) };
        Ok(parse_images(data))
    }
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    data: serde_json::Value,
}

#[derive(Deserialize)]
struct SearchGame {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
struct Image {
    url: String,
    thumb: String,
    #[serde(default)]
    score: i64,
    #[serde(default)]
    style: serde_json::Value,
}

fn parse_images(data: serde_json::Value) -> Vec<ArtOption> {
    let images: Vec<Image> = serde_json::from_value(data).unwrap_or_default();
    images
        .into_iter()
        .map(|i| ArtOption {
            url: i.url,
            thumb: i.thumb,
            score: i.score,
            // Older responses give a string, newer ones a list.
            style: match i.style {
                serde_json::Value::String(s) => s,
                serde_json::Value::Array(a) => a.first().and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                _ => String::new(),
            },
        })
        .collect()
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_image_lists_in_both_style_shapes() {
        let data: serde_json::Value = serde_json::from_str(
            r#"[{"id":1,"score":12,"style":"alternate","url":"https://cdn2.steamgriddb.com/grid/a.png","thumb":"https://cdn2.steamgriddb.com/thumb/a.jpg","width":600,"height":900},
                {"id":2,"score":3,"style":["material"],"url":"https://cdn2.steamgriddb.com/grid/b.png","thumb":"https://cdn2.steamgriddb.com/thumb/b.jpg"}]"#,
        )
        .unwrap();
        let imgs = parse_images(data);
        assert_eq!(imgs.len(), 2);
        assert_eq!((imgs[0].score, imgs[0].style.as_str()), (12, "alternate"));
        assert_eq!(imgs[1].style, "material");
        assert!(imgs[0].url.ends_with("a.png"));
    }

    #[test]
    fn cover_requests_tall_safe_still_images() {
        let f = ArtKind::Cover.filters();
        assert!(f.contains(&("dimensions", "600x900")));
        assert!(f.contains(&("nsfw", "false")) && f.contains(&("types", "static")));
        assert_eq!(urlencode("Assassin's Creed"), "Assassin%27s%20Creed");
    }
}
