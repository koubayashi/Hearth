//! The GOG account: every game it owns, with names and art. Signs in through
//! GOG Galaxy's public client, as Heroic and gogdl do: GOG's login page
//! redirects with a one-time code, traded for a refresh token.
//!
//! Names and game-or-DLC come from GOG's public product list, art from GOG's
//! public games database; both need no sign-in and are cached on disk, so a
//! sync only asks about games it hasn't seen. GOG doesn't publish playtime
//! outside Galaxy, so there is none.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::Locked;

const CLIENT_ID: &str = "46899977096215655";
const CLIENT_SECRET: &str = "9d85c43b1482497dbbce61f6e4aa173a433796eeae2ca8c5f6129f2dc4de46d9";
/// Where GOG sends the browser after sign-in, with `&code=…`.
pub const REDIRECT: &str = "https://embed.gog.com/on_login_success";
const REDIRECT_FULL: &str = "https://embed.gog.com/on_login_success?origin=client";

/// The sign-in was rejected or ran out; a fresh sign-in is needed.
pub const SESSION_EXPIRED: &str = "Your GOG sign-in expired. Sign in again in Settings.";

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn login_url() -> String {
    format!(
        "https://auth.gog.com/auth?client_id={CLIENT_ID}&redirect_uri={}&response_type=code&layout=client2",
        encode(REDIRECT_FULL)
    )
}

/// The one-time code from the address GOG redirects to after sign-in.
pub fn code_from_url(url: &str) -> Option<String> {
    let query = url.strip_prefix(REDIRECT)?.split_once('?')?.1;
    query.split('&').find_map(|pair| pair.strip_prefix("code=")).map(String::from).filter(|c| !c.is_empty())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
    /// Unix seconds.
    pub expires_at_unix: i64,
    #[serde(default)]
    pub username: Option<String>,
}

pub type SessionHook = Box<dyn Fn(Option<&Session>) + Send + Sync>;

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    user_id: String,
    expires_in: i64,
}

/// What Hearth keeps about a GOG product.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Product {
    pub title: String,
    pub is_game: bool,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub hero: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
    /// The games database has been asked for art (found or not).
    #[serde(default)]
    pub art_checked: bool,
    #[serde(default)]
    pub description: Option<String>,
    /// Fetched since descriptions were kept; older cache entries are fetched
    /// once more.
    #[serde(default)]
    pub described: bool,
}

#[derive(Deserialize)]
struct ApiProduct {
    id: u64,
    title: String,
    #[serde(default)]
    game_type: String,
    #[serde(default)]
    description: Option<ApiDescription>,
}

#[derive(Deserialize)]
struct ApiDescription {
    #[serde(default)]
    lead: String,
    #[serde(default)]
    full: String,
}

#[derive(Deserialize)]
struct GamesDbRelease {
    #[serde(default)]
    game: Option<GamesDbGame>,
}

#[derive(Deserialize)]
struct GamesDbGame {
    #[serde(default)]
    vertical_cover: Option<GamesDbImage>,
    #[serde(default)]
    background: Option<GamesDbImage>,
    #[serde(default)]
    logo: Option<GamesDbImage>,
}

#[derive(Deserialize)]
struct GamesDbImage {
    url_format: String,
}

/// GOG's image addresses are templates. The plain JPEG is sometimes a TIFF,
/// which the web view can't draw; WebP is right and small (a cover is about
/// 13 KB, a background about 240 KB).
fn image(i: Option<GamesDbImage>) -> Option<String> {
    let url = i?.url_format.replace("{formatter}", "").replace("{ext}", "webp");
    url.starts_with("https://").then_some(url)
}

pub struct GogWeb {
    http: reqwest::Client,
    session: Mutex<Option<Session>>,
    on_session: SessionHook,
    renewing: tokio::sync::Mutex<()>,
    catalog: Mutex<HashMap<String, Product>>,
    catalog_file: PathBuf,
}

impl GogWeb {
    /// `data_dir` holds the product cache; `on_session` saves sign-in changes.
    pub fn new(data_dir: &Path, session: Option<Session>, on_session: SessionHook) -> Self {
        let catalog_file = data_dir.join("gog-catalog.json");
        let catalog = std::fs::read_to_string(&catalog_file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Self {
            http: reqwest::Client::builder().user_agent("Hearth/0.1").build().expect("http client"),
            session: Mutex::new(session),
            on_session,
            renewing: tokio::sync::Mutex::new(()),
            catalog: Mutex::new(catalog),
            catalog_file,
        }
    }

    pub fn signed_in(&self) -> bool {
        self.session.locked().is_some()
    }

    pub fn username(&self) -> Option<String> {
        self.session.locked().as_ref().and_then(|s| s.username.clone())
    }

    pub fn forget(&self) {
        *self.session.locked() = None;
        (self.on_session)(None);
    }

    fn store(&self, s: Session) {
        (self.on_session)(Some(&s));
        *self.session.locked() = Some(s);
    }

    pub fn product(&self, id: &str) -> Option<Product> {
        self.catalog.locked().get(id).cloned()
    }

    /// Trades the one-time code for a session, then reads the username.
    pub async fn sign_in(&self, code: &str) -> anyhow::Result<()> {
        let mut s = self
            .token(&format!("grant_type=authorization_code&code={}&redirect_uri={}", encode(code), encode(REDIRECT_FULL)), None)
            .await?;
        s.username = self.username_for(&s).await;
        self.store(s);
        Ok(())
    }

    async fn username_for(&self, s: &Session) -> Option<String> {
        #[derive(Deserialize)]
        struct UserData {
            username: Option<String>,
        }
        let res = self.http.get("https://embed.gog.com/userData.json").bearer_auth(&s.access_token).send().await.ok()?;
        res.json::<UserData>().await.ok()?.username.filter(|u| !u.is_empty())
    }

    async fn token(&self, grant: &str, username: Option<String>) -> anyhow::Result<Session> {
        let url = format!("https://auth.gog.com/token?client_id={CLIENT_ID}&client_secret={CLIENT_SECRET}&{grant}");
        // The address holds the refresh token: errors must not carry it into
        // the log or onto the screen.
        let res = self.http.get(url).send().await.map_err(reqwest::Error::without_url)?;
        if matches!(res.status().as_u16(), 400 | 401) {
            bail!(SESSION_EXPIRED);
        }
        if !res.status().is_success() {
            bail!("GOG sign-in failed ({}). Try again later.", res.status());
        }
        let t: TokenResponse = res.json().await.map_err(reqwest::Error::without_url)?;
        Ok(Session {
            access_token: t.access_token,
            refresh_token: t.refresh_token,
            user_id: t.user_id,
            expires_at_unix: now() + t.expires_in,
            username,
        })
    }

    /// A current access token, renewed when about to run out. A rejected
    /// renewal means the sign-in is gone (SESSION_EXPIRED); anything else
    /// (offline) keeps it for next time.
    async fn access(&self) -> anyhow::Result<Session> {
        let Some(s) = self.session.locked().clone() else { bail!("Not signed in to GOG") };
        if s.expires_at_unix - now() > 120 {
            return Ok(s);
        }
        let _turn = self.renewing.lock().await;
        match self.session.locked().clone() {
            Some(c) if c.refresh_token != s.refresh_token => return Ok(c),
            None => bail!("Not signed in to GOG"),
            _ => {}
        }
        let fresh = self.token(&format!("grant_type=refresh_token&refresh_token={}", encode(&s.refresh_token)), s.username.clone()).await?;
        if !self.replace(&s.refresh_token, &fresh) {
            bail!("Not signed in to GOG");
        }
        Ok(fresh)
    }

    /// Saves a renewed session, unless the user signed out (or signed in
    /// again) while it was being fetched: a sign-out must stay a sign-out.
    fn replace(&self, stale_refresh: &str, fresh: &Session) -> bool {
        let mut current = self.session.locked();
        if current.as_ref().is_none_or(|c| c.refresh_token != stale_refresh) {
            return false;
        }
        (self.on_session)(Some(fresh));
        *current = Some(fresh.clone());
        true
    }

    pub async fn keep_alive(&self) -> anyhow::Result<()> {
        self.access().await.map(drop)
    }

    /// Ids of every product the account owns (games and DLC).
    pub async fn owned_ids(&self) -> anyhow::Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Owned {
            owned: Vec<u64>,
        }
        let s = self.access().await?;
        let res = self.http.get("https://embed.gog.com/user/data/games").bearer_auth(&s.access_token).send().await?;
        if res.status().as_u16() == 401 {
            bail!(SESSION_EXPIRED);
        }
        if !res.status().is_success() {
            bail!("GOG returned {} for the game list", res.status());
        }
        Ok(res.json::<Owned>().await?.owned.into_iter().map(|id| id.to_string()).collect())
    }

    /// Names and art for these products, from the cache or GOG's public
    /// services for any not seen before. Failures leave a product unknown
    /// until the next sync rather than failing the sync.
    pub async fn describe(&self, ids: &[String]) {
        let unknown: Vec<String> = {
            let c = self.catalog.locked();
            ids.iter().filter(|id| !c.get(*id).is_some_and(|p| p.described)).cloned().collect()
        };
        for batch in unknown.chunks(50) {
            let url = format!("https://api.gog.com/products?ids={}&expand=description", batch.join(","));
            let Ok(res) = self.http.get(url).send().await else { continue };
            let Ok(products) = res.json::<Vec<ApiProduct>>().await else { continue };
            let mut c = self.catalog.locked();
            for p in products {
                let description = p
                    .description
                    .map(|d| crate::metadata::plain_text(if d.lead.trim().is_empty() { &d.full } else { &d.lead }))
                    .filter(|d| !d.is_empty());
                // Art already found is kept; only the text is new.
                let old = c.remove(&p.id.to_string()).unwrap_or_default();
                c.insert(
                    p.id.to_string(),
                    Product { title: p.title, is_game: p.game_type == "game", description, described: true, ..old },
                );
            }
        }
        let need_art: Vec<String> = {
            let c = self.catalog.locked();
            ids.iter().filter(|id| c.get(*id).is_some_and(|p| p.is_game && !p.art_checked)).cloned().collect()
        };
        for id in need_art {
            let url = format!("https://gamesdb.gog.com/platforms/gog/external_releases/{id}");
            let game = match self.http.get(url).send().await {
                Ok(r) if r.status().is_success() => r.json::<GamesDbRelease>().await.ok().and_then(|r| r.game),
                Ok(_) => None,
                Err(_) => continue,
            };
            let mut c = self.catalog.locked();
            if let Some(p) = c.get_mut(&id) {
                if let Some(g) = game {
                    p.cover = image(g.vertical_cover);
                    p.hero = image(g.background);
                    p.logo = image(g.logo);
                }
                p.art_checked = true;
            }
        }
        let json = serde_json::to_string(&*self.catalog.locked());
        if let Ok(json) = json {
            let _ = std::fs::write(&self.catalog_file, json);
        }
    }
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_url_redirects_to_galaxy_page() {
        let url = login_url();
        assert!(url.starts_with("https://auth.gog.com/auth?client_id=46899977096215655&redirect_uri=https%3A%2F%2Fembed.gog.com"));
        assert!(url.ends_with("&response_type=code&layout=client2"));
    }

    #[test]
    fn finds_the_code_in_the_redirect() {
        assert_eq!(code_from_url(&format!("{REDIRECT_FULL}&code=abc-123_x")).as_deref(), Some("abc-123_x"));
        assert_eq!(code_from_url(REDIRECT_FULL), None);
        assert_eq!(code_from_url("https://auth.gog.com/auth?code=x"), None);
    }

    #[test]
    fn fills_image_templates_as_webp() {
        let i = GamesDbImage { url_format: "https://images.gog.com/abc{formatter}.{ext}?namespace=gamesdb".into() };
        assert_eq!(image(Some(i)).as_deref(), Some("https://images.gog.com/abc.webp?namespace=gamesdb"));
        assert_eq!(image(None), None);
    }
}
