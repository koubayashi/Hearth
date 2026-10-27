//! The Xbox account: games played on it (PC or console), their playtime and
//! art, and the gamertag. Signs in with a Microsoft account through the same
//! public client and endpoints as Playnite's Xbox plugin: Microsoft's OAuth
//! gives a refresh token, which is traded for an Xbox user token and then an
//! XSTS token that the Xbox services accept.
//!
//! Xbox lists the games the account has played, not every purchase; a game
//! bought or on Game Pass but never started doesn't appear until it's played.

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::json;

const CLIENT_ID: &str = "38cd2fa8-66fd-4760-afb2-405eb65d5b0c";
const SCOPE: &str = "Xboxlive.signin Xboxlive.offline_access";
/// Where Microsoft sends the browser after sign-in, with `?code=…`.
pub const REDIRECT: &str = "https://login.live.com/oauth20_desktop.srf";
const TOKEN_URL: &str = "https://login.live.com/oauth20_token.srf";

/// The sign-in was rejected or ran out; a fresh sign-in is needed.
pub const SESSION_EXPIRED: &str = "Your Xbox sign-in expired. Sign in again in Settings.";

pub fn login_url() -> String {
    let q = [
        ("client_id", CLIENT_ID),
        ("response_type", "code"),
        ("approval_prompt", "auto"),
        ("scope", SCOPE),
        ("redirect_uri", REDIRECT),
    ];
    let query: Vec<String> = q.iter().map(|(k, v)| format!("{k}={}", encode(v))).collect();
    format!("https://login.live.com/oauth20_authorize.srf?{}", query.join("&"))
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The one-time code from the address Microsoft redirects to after sign-in.
pub fn code_from_url(url: &str) -> Option<String> {
    let rest = url.strip_prefix(REDIRECT)?;
    let query = rest.strip_prefix('?')?;
    query.split('&').find_map(|pair| pair.strip_prefix("code=")).map(String::from).filter(|c| !c.is_empty())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix seconds.
    pub expires_at_unix: i64,
    #[serde(default)]
    pub gamertag: Option<String>,
}

pub type SessionHook = Box<dyn Fn(Option<&Session>) + Send + Sync>;

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

/// What the Xbox services take: `XBL3.0 x=<uhs>;<token>`, and the user's ids.
#[derive(Clone)]
struct Xsts {
    header: String,
    xuid: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxToken {
    token: String,
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<Xui>,
}

#[derive(Deserialize)]
struct Xui {
    uhs: String,
    #[serde(default)]
    xid: Option<String>,
    #[serde(default)]
    gtg: Option<String>,
}

/// A game the account has played, as Xbox lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayedTitle {
    /// The Store package family, the same id the installed scan uses.
    pub pfn: String,
    pub title_id: String,
    pub name: String,
    /// Unix seconds.
    pub last_played: Option<i64>,
    pub cover: Option<String>,
    pub hero: Option<String>,
    pub logo: Option<String>,
}

#[derive(Deserialize)]
struct TitleHistory {
    #[serde(default)]
    titles: Vec<Title>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Title {
    title_id: String,
    #[serde(default)]
    pfn: Option<String>,
    #[serde(rename = "type", default)]
    kind: String,
    name: String,
    #[serde(default)]
    devices: Vec<String>,
    #[serde(default)]
    title_history: Option<TitleHistoryEntry>,
    #[serde(default)]
    images: Vec<Image>,
    #[serde(default)]
    display_image: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TitleHistoryEntry {
    #[serde(default)]
    last_time_played: Option<String>,
}

#[derive(Deserialize)]
struct Image {
    url: String,
    #[serde(rename = "type")]
    kind: String,
}

/// PC games only: Xbox also lists console games and apps the account used.
fn played_titles(history: TitleHistory) -> Vec<PlayedTitle> {
    history
        .titles
        .into_iter()
        .filter(|t| t.kind == "Game" && t.devices.iter().any(|d| d == "PC"))
        .filter_map(|t| {
            let pfn = t.pfn.filter(|p| !p.is_empty())?;
            let image = |kinds: &[&str]| {
                kinds.iter().find_map(|k| t.images.iter().find(|i| i.kind.eq_ignore_ascii_case(k))).map(|i| https(&i.url))
            };
            let fit = |url: Option<String>, slot| url.map(|u| sized(&u, slot).unwrap_or(u));
            Some(PlayedTitle {
                cover: fit(image(&["Poster", "BoxArt"]).or_else(|| t.display_image.as_deref().map(https)), "cover"),
                hero: fit(image(&["SuperHeroArt", "TitledHeroArt", "Screenshot"]), "hero"),
                logo: fit(image(&["Logo"]), "logo"),
                last_played: t.title_history.and_then(|h| h.last_time_played).as_deref().and_then(unix_from_iso),
                title_id: t.title_id,
                name: t.name,
                pfn,
            })
        })
        .collect()
}

/// Microsoft's image service sends the original file unless asked for a size:
/// backgrounds are 4K PNGs of around 15 MB, and some covers are TIFF, which
/// the web view can't draw. With a width it sends a real JPEG of a sensible
/// size (a background drops to about 330 KB). Logos stay PNG for their
/// transparency. Addresses from elsewhere, or already sized, are unchanged.
pub fn sized(url: &str, slot: &str) -> Option<String> {
    if !url.starts_with("https://store-images.s-microsoft.com/") || url.contains('?') {
        return None;
    }
    Some(match slot {
        "cover" => format!("{url}?w=600&format=jpg"),
        "hero" => format!("{url}?w=1920&q=85&format=jpg"),
        _ => format!("{url}?w=800"),
    })
}

/// Xbox image addresses sometimes come without a scheme or over http.
fn https(url: &str) -> String {
    match url {
        u if u.starts_with("//") => format!("https:{u}"),
        u if u.starts_with("http://") => format!("https://{}", &u[7..]),
        u => u.to_string(),
    }
}

/// "2024-03-01T12:34:56.789Z" → unix seconds (UTC; fractions ignored).
pub fn unix_from_iso(s: &str) -> Option<i64> {
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let time = time.trim_end_matches('Z');
    let time = time.split(['+', '.']).next()?;
    let mut t = time.split(':').map(|p| p.parse::<i64>().ok());
    let (h, min, sec) = (t.next()??, t.next()??, t.next().flatten().unwrap_or(0));
    // Days since 1970-01-01 (Howard Hinnant's days_from_civil).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days * 86400 + h * 3600 + min * 60 + sec)
}

#[derive(Deserialize)]
struct UserStats {
    #[serde(default)]
    statlistscollection: Vec<StatList>,
}

#[derive(Deserialize)]
struct StatList {
    #[serde(default)]
    stats: Vec<Stat>,
}

#[derive(Deserialize)]
struct Stat {
    titleid: String,
    #[serde(default)]
    value: Option<String>,
}

/// Minutes played per title id, as seconds.
fn playtime(stats: UserStats) -> HashMap<String, i64> {
    stats
        .statlistscollection
        .into_iter()
        .flat_map(|l| l.stats)
        .filter_map(|s| Some((s.titleid, s.value?.parse::<i64>().ok()? * 60)))
        .collect()
}

pub struct XboxLive {
    http: reqwest::Client,
    session: Mutex<Option<Session>>,
    on_session: SessionHook,
    /// One renewal at a time: Microsoft replaces the refresh token each time.
    renewing: tokio::sync::Mutex<()>,
}

impl XboxLive {
    pub fn new(session: Option<Session>, on_session: SessionHook) -> Self {
        Self {
            http: reqwest::Client::builder().user_agent("Hearth/0.1").build().expect("http client"),
            session: Mutex::new(session),
            on_session,
            renewing: tokio::sync::Mutex::new(()),
        }
    }

    pub fn signed_in(&self) -> bool {
        self.session.lock().unwrap().is_some()
    }

    pub fn gamertag(&self) -> Option<String> {
        self.session.lock().unwrap().as_ref().and_then(|s| s.gamertag.clone())
    }

    pub fn forget(&self) {
        *self.session.lock().unwrap() = None;
        (self.on_session)(None);
    }

    fn store(&self, s: Session) {
        (self.on_session)(Some(&s));
        *self.session.lock().unwrap() = Some(s);
    }

    /// Trades the one-time code for a session, then reads the gamertag.
    pub async fn sign_in(&self, code: &str) -> anyhow::Result<()> {
        let s = self.token(&[("grant_type", "authorization_code"), ("code", code)], None).await?;
        self.store(s);
        self.xsts().await?;
        Ok(())
    }

    async fn token(&self, form: &[(&str, &str)], gamertag: Option<String>) -> anyhow::Result<Session> {
        let mut body = form.to_vec();
        body.extend([("client_id", CLIENT_ID), ("scope", SCOPE), ("redirect_uri", REDIRECT)]);
        let res = self.http.post(TOKEN_URL).form(&body).send().await?;
        if matches!(res.status().as_u16(), 400 | 401) {
            bail!(SESSION_EXPIRED);
        }
        if !res.status().is_success() {
            bail!("Microsoft sign-in failed ({}). Try again later.", res.status());
        }
        let t: TokenResponse = res.json().await?;
        Ok(Session {
            access_token: t.access_token,
            refresh_token: t.refresh_token,
            expires_at_unix: now() + t.expires_in,
            gamertag,
        })
    }

    /// A current Microsoft access token, renewed when about to run out. A
    /// rejected renewal means the sign-in itself is gone (SESSION_EXPIRED);
    /// anything else (offline) keeps it for next time.
    async fn access(&self) -> anyhow::Result<Session> {
        let Some(s) = self.session.lock().unwrap().clone() else { bail!("Not signed in to Xbox") };
        if s.expires_at_unix - now() > 120 {
            return Ok(s);
        }
        let _turn = self.renewing.lock().await;
        if let Some(c) = self.session.lock().unwrap().clone() {
            if c.refresh_token != s.refresh_token {
                return Ok(c);
            }
        }
        let fresh = self.token(&[("grant_type", "refresh_token"), ("refresh_token", &s.refresh_token)], s.gamertag).await?;
        self.store(fresh.clone());
        Ok(fresh)
    }

    /// Renews ahead of time, so a refresh token used at least every 90 days
    /// never runs out while Hearth is in the tray.
    pub async fn keep_alive(&self) -> anyhow::Result<()> {
        self.access().await.map(drop)
    }

    async fn xsts(&self) -> anyhow::Result<Xsts> {
        let s = self.access().await?;
        let user: XboxToken = self
            .post_json(
                "https://user.auth.xboxlive.com/user/authenticate",
                &json!({
                    "RelyingParty": "http://auth.xboxlive.com",
                    "TokenType": "JWT",
                    "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={}", s.access_token) },
                }),
            )
            .await
            .context("Xbox didn't accept the Microsoft sign-in")?;
        let xsts: XboxToken = self
            .post_json(
                "https://xsts.auth.xboxlive.com/xsts/authorize",
                &json!({
                    "RelyingParty": "http://xboxlive.com",
                    "TokenType": "JWT",
                    "Properties": { "SandboxId": "RETAIL", "UserTokens": [user.token] },
                }),
            )
            .await
            .context("This Microsoft account has no Xbox profile yet. Sign in to the Xbox app once to create one")?;
        let claims = xsts.display_claims.xui.into_iter().next().context("Xbox sent no account details")?;
        let xuid = claims.xid.context("Xbox sent no account id")?;
        if claims.gtg.is_some() && claims.gtg != s.gamertag {
            let mut current = self.session.lock().unwrap().clone().unwrap_or(s);
            current.gamertag = claims.gtg.clone();
            self.store(current);
        }
        Ok(Xsts { header: format!("XBL3.0 x={};{}", claims.uhs, xsts.token), xuid })
    }

    async fn post_json<T: serde::de::DeserializeOwned>(&self, url: &str, body: &serde_json::Value) -> anyhow::Result<T> {
        let res = self.http.post(url).header("x-xbl-contract-version", "1").json(body).send().await?;
        if !res.status().is_success() {
            bail!("{} returned {}", url, res.status());
        }
        Ok(res.json().await?)
    }

    /// PC games the account has played, with playtime in seconds by title id.
    pub async fn library(&self) -> anyhow::Result<(Vec<PlayedTitle>, HashMap<String, i64>)> {
        let x = self.xsts().await?;
        let url = format!(
            "https://titlehub.xboxlive.com/users/xuid({})/titles/titlehistory/decoration/detail,image",
            x.xuid
        );
        let res = self
            .http
            .get(&url)
            .header("Authorization", &x.header)
            .header("x-xbl-contract-version", "2")
            .header("Accept-Language", "en-US")
            .send()
            .await?;
        if !res.status().is_success() {
            bail!("Xbox returned {} for the game list", res.status());
        }
        let titles = played_titles(res.json().await?);

        // Playtime is a separate service; without it the games still show.
        let stats = json!({
            "arrangebyfield": "xuid",
            "xuids": [x.xuid],
            "stats": titles.iter().map(|t| json!({ "name": "MinutesPlayed", "titleid": t.title_id })).collect::<Vec<_>>(),
        });
        let minutes = match self
            .http
            .post("https://userstats.xboxlive.com/batch")
            .header("Authorization", &x.header)
            .header("x-xbl-contract-version", "2")
            .json(&stats)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r.json::<UserStats>().await.map(playtime).unwrap_or_default(),
            _ => HashMap::new(),
        };
        Ok((titles, minutes))
    }
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_url_asks_for_offline_access() {
        let url = login_url();
        assert!(url.starts_with("https://login.live.com/oauth20_authorize.srf?client_id=38cd2fa8"));
        assert!(url.contains("scope=Xboxlive.signin%20Xboxlive.offline_access"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Flogin.live.com%2Foauth20_desktop.srf"));
    }

    #[test]
    fn finds_the_code_in_the_redirect() {
        assert_eq!(code_from_url(&format!("{REDIRECT}?code=M.C5_BAY.2.abc&lc=1033")).as_deref(), Some("M.C5_BAY.2.abc"));
        assert_eq!(code_from_url(&format!("{REDIRECT}?error=access_denied")), None);
        assert_eq!(code_from_url("https://login.live.com/other?code=x"), None);
    }

    #[test]
    fn keeps_pc_games_only() {
        let history: TitleHistory = serde_json::from_value(json!({
            "titles": [
                { "titleId": "1", "pfn": "Microsoft.624F8B84B80_8wekyb3d8bbwe", "type": "Game", "name": "Forza Horizon 5",
                  "devices": ["PC", "XboxSeries"], "titleHistory": { "lastTimePlayed": "2026-10-01T20:15:30.1234567Z" },
                  "images": [ { "url": "http://store-images.s-microsoft.com/poster", "type": "Poster" },
                              { "url": "//store-images.s-microsoft.com/hero", "type": "SuperHeroArt" } ] },
                { "titleId": "2", "pfn": "Some.Console_x", "type": "Game", "name": "Console only", "devices": ["XboxOne"] },
                { "titleId": "3", "pfn": "Microsoft.Netflix_x", "type": "App", "name": "Netflix", "devices": ["PC"] },
                { "titleId": "4", "type": "Game", "name": "No package", "devices": ["PC"] }
            ]
        }))
        .unwrap();
        let games = played_titles(history);
        assert_eq!(games.len(), 1);
        let g = &games[0];
        assert_eq!(g.pfn, "Microsoft.624F8B84B80_8wekyb3d8bbwe");
        assert_eq!(g.cover.as_deref(), Some("https://store-images.s-microsoft.com/poster?w=600&format=jpg"));
        assert_eq!(g.hero.as_deref(), Some("https://store-images.s-microsoft.com/hero?w=1920&q=85&format=jpg"));
        assert_eq!(g.last_played, Some(1_790_885_730));
    }

    #[test]
    fn reads_minutes_played() {
        let stats: UserStats = serde_json::from_value(json!({
            "statlistscollection": [ { "stats": [ { "titleid": "1", "value": "125" }, { "titleid": "2" } ] } ]
        }))
        .unwrap();
        assert_eq!(playtime(stats), HashMap::from([("1".to_string(), 7500)]));
    }

    #[test]
    fn parses_iso_dates() {
        assert_eq!(unix_from_iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix_from_iso("2000-03-01T00:00:00Z"), Some(951_868_800));
        assert_eq!(unix_from_iso("not a date"), None);
    }
}
