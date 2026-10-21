//! Epic Games: installed games from the launcher's local manifests, plus the
//! owned library and playtime from Epic's web services once signed in.
//!
//! Uses the same launcher OAuth client and endpoints as Playnite's Epic plugin
//! and legendary: sign in on epicgames.com, trade the one-time authorization
//! code for tokens, then read library items, playtime and catalog details.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, bail, Context};
use async_trait::async_trait;
use hub_core::{GameEntry, StoreId};
use hub_launch::LaunchAction;
use serde::{Deserialize, Serialize};

use crate::{Artwork, LibraryProvider};

const CLIENT_ID: &str = "34a02cf8f4414e29b15921876da36f9a";
/// base64("34a02cf8f4414e29b15921876da36f9a:daafbccc737745039dffe53d94fc76cf"),
/// the Epic Games Launcher's public client credentials.
const CLIENT_AUTH: &str = "basic MzRhMDJjZjhmNDQxNGUyOWIxNTkyMTg3NmRhMzZmOWE6ZGFhZmJjY2M3Mzc3NDUwMzlkZmZlNTNkOTRmYzc2Y2Y=";
const OAUTH_URL: &str = "https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/token";
const LIBRARY_URL: &str =
    "https://library-service.live.use1a.on.epicgames.com/library/api/public/items?includeMetadata=true&platform=Windows";
const PLAYTIME_URL: &str = "https://library-service.live.use1a.on.epicgames.com/library/api/public/playtime/account";
const CATALOG_URL: &str = "https://catalog-public-service-prod06.ol.epicgames.com/catalog/api/shared/namespace";

/// Where the user signs in. After login Epic shows a JSON page containing
/// `authorizationCode`, which the app's login window picks up.
pub fn login_url() -> String {
    let redirect = format!("https://www.epicgames.com/id/api/redirect?clientId={CLIENT_ID}&responseType=code");
    format!(
        "https://www.epicgames.com/id/login?redirectUrl={}",
        redirect.replace(':', "%3A").replace('/', "%2F").replace('?', "%3F").replace('=', "%3D").replace('&', "%26")
    )
}

/// Pulls the code out of the redirect page's JSON body, if present.
pub fn code_from_redirect_body(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Redirect {
        authorization_code: Option<String>,
    }
    serde_json::from_str::<Redirect>(body.trim()).ok()?.authorization_code.filter(|c| !c.is_empty())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    pub account_id: String,
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
    /// Unix seconds.
    pub expires_at_unix: i64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    account_id: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    expires_in: i64,
}

/// Game identity in Epic: all three parts are needed to launch.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EpicId {
    pub namespace: String,
    pub catalog_item_id: String,
    pub app_name: String,
}

impl EpicId {
    pub fn encode(&self) -> String {
        format!("{}:{}:{}", self.namespace, self.catalog_item_id, self.app_name)
    }

    pub fn decode(s: &str) -> Option<Self> {
        let mut it = s.splitn(3, ':');
        Some(Self {
            namespace: it.next()?.to_string(),
            catalog_item_id: it.next()?.to_string(),
            app_name: it.next()?.to_string(),
        })
    }

    fn launcher_uri(&self, action: &str) -> String {
        format!(
            "com.epicgames.launcher://apps/{}%3A{}%3A{}?action={action}&silent=true",
            self.namespace, self.catalog_item_id, self.app_name
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CatalogEntry {
    title: String,
    is_game: bool,
    cover: Option<String>,
    hero: Option<String>,
    logo: Option<String>,
}

pub type SessionHook = Box<dyn Fn(Option<&Session>) + Send + Sync>;

pub struct Epic {
    manifests: PathBuf,
    catalog_cache: PathBuf,
    http: reqwest::Client,
    session: Mutex<Option<Session>>,
    catalog: Mutex<HashMap<String, CatalogEntry>>,
    on_session: SessionHook,
}

impl Epic {
    /// `data_dir` holds the catalog cache; `on_session` persists sign-in changes.
    pub fn new(data_dir: &Path, session: Option<Session>, on_session: SessionHook) -> Self {
        let catalog_cache = data_dir.join("epic-catalog.json");
        let catalog = std::fs::read_to_string(&catalog_cache)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Self {
            manifests: PathBuf::from(r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests"),
            catalog_cache,
            http: reqwest::Client::builder()
                .user_agent("Hearth/0.1 (+https://github.com)")
                .build()
                .expect("http client"),
            session: Mutex::new(session),
            catalog: Mutex::new(catalog),
            on_session,
        }
    }

    pub fn with_manifests(mut self, dir: impl Into<PathBuf>) -> Self {
        self.manifests = dir.into();
        self
    }

    pub fn account_name(&self) -> Option<String> {
        self.session.lock().unwrap().as_ref().map(|s| s.display_name.clone().unwrap_or_else(|| s.account_id.clone()))
    }

    /// Forgets the session locally and revokes it on Epic's side.
    pub async fn sign_out(&self) {
        let old = self.session.lock().unwrap().take();
        (self.on_session)(None);
        if let Some(s) = old {
            let url = format!(
                "https://account-public-service-prod03.ol.epicgames.com/account/api/oauth/sessions/kill/{}",
                s.access_token
            );
            let _ = self.http.delete(url).bearer_auth(&s.access_token).send().await;
        }
    }

    fn drop_session(&self) {
        *self.session.lock().unwrap() = None;
        (self.on_session)(None);
    }

    /// Exchanges the one-time code from the login page for a session.
    pub async fn sign_in(&self, code: &str) -> anyhow::Result<()> {
        let s = self.token(&[("grant_type", "authorization_code"), ("code", code.trim())]).await?;
        self.store(s);
        Ok(())
    }

    async fn token(&self, form: &[(&str, &str)]) -> anyhow::Result<Session> {
        let mut body: Vec<(&str, &str)> = form.to_vec();
        body.push(("token_type", "eg1"));
        let res = self
            .http
            .post(OAUTH_URL)
            .header("Authorization", CLIENT_AUTH)
            .form(&body)
            .send()
            .await?;
        if !res.status().is_success() {
            bail!("Epic sign-in failed ({}). The code may have expired; sign in again.", res.status());
        }
        let t: TokenResponse = res.json().await?;
        Ok(Session {
            access_token: t.access_token,
            refresh_token: t.refresh_token,
            account_id: t.account_id,
            display_name: t.display_name,
            expires_at_unix: now() + t.expires_in,
        })
    }

    fn store(&self, s: Session) {
        (self.on_session)(Some(&s));
        *self.session.lock().unwrap() = Some(s);
    }

    /// A valid access token, refreshing it when it is about to expire.
    async fn access(&self) -> anyhow::Result<Option<Session>> {
        let Some(s) = self.session.lock().unwrap().clone() else { return Ok(None) };
        if s.expires_at_unix - now() > 120 {
            return Ok(Some(s));
        }
        match self.token(&[("grant_type", "refresh_token"), ("refresh_token", &s.refresh_token)]).await {
            Ok(fresh) => {
                self.store(fresh.clone());
                Ok(Some(fresh))
            }
            Err(e) => {
                self.drop_session();
                Err(e.context("Your Epic sign-in expired; sign in again in Settings"))
            }
        }
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str, s: &Session) -> anyhow::Result<T> {
        let res = self.http.get(url).bearer_auth(&s.access_token).send().await?;
        if !res.status().is_success() {
            bail!("Epic returned {} for {}", res.status(), url.split('?').next().unwrap_or(url));
        }
        Ok(res.json().await?)
    }

    async fn library_records(&self, s: &Session) -> anyhow::Result<Vec<LibraryRecord>> {
        let mut out = Vec::new();
        let mut url = LIBRARY_URL.to_string();
        loop {
            let page: LibraryPage = self.get(&url, s).await?;
            out.extend(page.records);
            match page.response_metadata.and_then(|m| m.next_cursor) {
                Some(c) if !c.is_empty() => url = format!("{LIBRARY_URL}&cursor={c}"),
                _ => break,
            }
        }
        Ok(out)
    }

    async fn playtime(&self, s: &Session) -> anyhow::Result<HashMap<String, i64>> {
        let items: Vec<PlaytimeItem> = self.get(&format!("{PLAYTIME_URL}/{}/all", s.account_id), s).await?;
        Ok(items.into_iter().map(|p| (p.artifact_id, p.total_time)).collect())
    }

    async fn fetch_catalog(&self, s: &Session, id: &EpicId) -> Option<(String, CatalogEntry)> {
        let url = format!(
            "{CATALOG_URL}/{}/bulk/items?id={}&country=US&locale=en-US&includeMainGameDetails=true",
            id.namespace, id.catalog_item_id
        );
        let items: HashMap<String, CatalogItem> = self.get(&url, s).await.ok()?;
        let item = items.get(&id.catalog_item_id)?;
        Some((id.catalog_item_id.clone(), catalog_entry(item)))
    }

    /// Fetch catalog details for items we haven't seen, 8 at a time, and cache them.
    async fn fill_catalog(&self, s: &Session, ids: &[EpicId]) {
        let missing: Vec<&EpicId> = {
            let cat = self.catalog.lock().unwrap();
            ids.iter().filter(|i| !cat.contains_key(&i.catalog_item_id)).collect()
        };
        if missing.is_empty() {
            return;
        }
        // Batches of 8 in flight; join_all keeps the futures free of closure
        // lifetimes, which async_trait's Send bound can't see through.
        let mut fetched = Vec::new();
        for chunk in missing.chunks(8) {
            let batch = futures::future::join_all(chunk.iter().map(|id| self.fetch_catalog(s, id))).await;
            fetched.extend(batch.into_iter().flatten());
        }
        let mut cat = self.catalog.lock().unwrap();
        cat.extend(fetched);
        if let Ok(json) = serde_json::to_string(&*cat) {
            let _ = std::fs::write(&self.catalog_cache, json);
        }
    }
}

#[async_trait]
impl LibraryProvider for Epic {
    fn id(&self) -> StoreId {
        StoreId::Epic
    }

    async fn installed(&self) -> anyhow::Result<Vec<GameEntry>> {
        let Ok(dir) = std::fs::read_dir(&self.manifests) else { return Ok(Vec::new()) };
        Ok(dir
            .flatten()
            .filter(|f| f.path().extension().is_some_and(|e| e == "item"))
            .filter_map(|f| std::fs::read_to_string(f.path()).ok())
            .filter_map(|t| parse_manifest(&t))
            .collect())
    }

    async fn owned(&self) -> anyhow::Result<Vec<GameEntry>> {
        let Some(s) = self.access().await? else { return Ok(Vec::new()) };
        let records = self.library_records(&s).await.context("reading your Epic library")?;
        let ids: Vec<EpicId> = records
            .into_iter()
            // "ue" is Unreal Engine marketplace content, not games.
            .filter(|r| r.namespace != "ue" && r.sandbox_type.as_deref() != Some("PRIVATE"))
            .map(|r| EpicId { namespace: r.namespace, catalog_item_id: r.catalog_item_id, app_name: r.app_name })
            .collect();
        self.fill_catalog(&s, &ids).await;
        let playtime = self.playtime(&s).await.unwrap_or_default();

        let cat = self.catalog.lock().unwrap();
        Ok(ids
            .into_iter()
            .filter_map(|id| {
                let c = cat.get(&id.catalog_item_id)?;
                c.is_game.then(|| GameEntry {
                    store: StoreId::Epic,
                    store_game_id: id.encode(),
                    title: c.title.clone(),
                    installed: false,
                    install_dir: None,
                    store_playtime_secs: playtime.get(&id.app_name).copied(),
                    store_last_played: None,
                })
            })
            .collect())
    }

    fn artwork(&self, store_game_id: &str) -> Artwork {
        let Some(id) = EpicId::decode(store_game_id) else { return Artwork::default() };
        let cat = self.catalog.lock().unwrap();
        cat.get(&id.catalog_item_id)
            .map(|c| Artwork { cover: c.cover.clone(), hero: c.hero.clone(), logo: c.logo.clone() })
            .unwrap_or_default()
    }

    fn launch(&self, store_game_id: &str) -> LaunchAction {
        let uri = EpicId::decode(store_game_id)
            .map(|id| id.launcher_uri("launch"))
            .unwrap_or_else(|| "com.epicgames.launcher://store".into());
        LaunchAction::Uri { uri }
    }

    fn install(&self, store_game_id: &str) -> Option<LaunchAction> {
        EpicId::decode(store_game_id).map(|id| LaunchAction::Uri { uri: id.launcher_uri("install") })
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Manifest {
    display_name: String,
    app_name: String,
    catalog_namespace: String,
    catalog_item_id: String,
    install_location: Option<String>,
    #[serde(default)]
    b_is_incomplete_install: bool,
    #[serde(default)]
    app_categories: Vec<String>,
    #[serde(default)]
    main_game_app_name: String,
}

fn parse_manifest(text: &str) -> Option<GameEntry> {
    let m: Manifest = serde_json::from_str(text).ok()?;
    // DLC manifests point at their main game; plugins and tools aren't "games".
    let is_dlc = !m.main_game_app_name.is_empty() && m.main_game_app_name != m.app_name;
    if is_dlc || !m.app_categories.iter().any(|c| c == "games") {
        return None;
    }
    let id = EpicId { namespace: m.catalog_namespace, catalog_item_id: m.catalog_item_id, app_name: m.app_name };
    Some(GameEntry {
        store: StoreId::Epic,
        store_game_id: id.encode(),
        title: m.display_name,
        installed: !m.b_is_incomplete_install,
        install_dir: m.install_location,
        store_playtime_secs: None,
        store_last_played: None,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryPage {
    #[serde(default)]
    records: Vec<LibraryRecord>,
    response_metadata: Option<ResponseMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseMetadata {
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryRecord {
    namespace: String,
    catalog_item_id: String,
    app_name: String,
    sandbox_type: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaytimeItem {
    artifact_id: String,
    total_time: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogItem {
    title: String,
    #[serde(default)]
    key_images: Vec<KeyImage>,
    #[serde(default)]
    categories: Vec<Category>,
    main_game_item: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct KeyImage {
    #[serde(rename = "type")]
    kind: String,
    url: String,
}

#[derive(Deserialize)]
struct Category {
    path: String,
}

fn catalog_entry(item: &CatalogItem) -> CatalogEntry {
    let img = |kinds: &[&str]| {
        kinds
            .iter()
            .find_map(|k| item.key_images.iter().find(|i| i.kind == *k))
            .map(|i| i.url.clone())
    };
    let is_game = item.main_game_item.is_none()
        && item.categories.iter().any(|c| c.path == "games")
        && !item.categories.iter().any(|c| c.path == "addons" || c.path == "digitalextras");
    CatalogEntry {
        title: item.title.clone(),
        is_game,
        cover: img(&["DieselGameBoxTall", "OfferImageTall", "Thumbnail"]),
        hero: img(&["DieselGameBox", "OfferImageWide", "DieselStoreFrontWide"]),
        logo: img(&["DieselGameBoxLogo"]),
    }
}

/// Fails loudly if a code was pasted with the surrounding JSON by mistake.
pub fn clean_code(input: &str) -> anyhow::Result<String> {
    if let Some(c) = code_from_redirect_body(input) {
        return Ok(c);
    }
    let c = input.trim().trim_matches('"');
    if c.len() < 16 || !c.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return Err(anyhow!("That doesn't look like an Epic authorization code"));
    }
    Ok(c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{
        "FormatVersion": 0, "bIsIncompleteInstall": false,
        "DisplayName": "Red Dead Redemption 2", "AppName": "Heather",
        "CatalogNamespace": "b30b6d1b4dfd4dcc93b5490be5e094e5",
        "CatalogItemId": "22a7b503221442daa2fb16ad37b6ccbf",
        "InstallLocation": "D:\\Games\\Epic Games\\RedDeadRedemption2",
        "AppCategories": ["public", "games", "applications"], "MainGameAppName": ""
    }"#;

    #[test]
    fn parses_installed_manifest_and_launch_uri() {
        let g = parse_manifest(MANIFEST).unwrap();
        assert_eq!(g.title, "Red Dead Redemption 2");
        assert!(g.installed);
        assert_eq!(g.install_dir.as_deref(), Some(r"D:\Games\Epic Games\RedDeadRedemption2"));
        let id = EpicId::decode(&g.store_game_id).unwrap();
        assert_eq!(id.app_name, "Heather");
        assert_eq!(
            id.launcher_uri("launch"),
            "com.epicgames.launcher://apps/b30b6d1b4dfd4dcc93b5490be5e094e5%3A22a7b503221442daa2fb16ad37b6ccbf%3AHeather?action=launch&silent=true"
        );
    }

    #[test]
    fn skips_dlc_and_non_games() {
        let dlc = MANIFEST.replace(r#""MainGameAppName": """#, r#""MainGameAppName": "Other""#);
        assert!(parse_manifest(&dlc).is_none());
        let tool = MANIFEST.replace(r#""public", "games", "applications""#, r#""public", "applications""#);
        assert!(parse_manifest(&tool).is_none());
    }

    #[test]
    fn reads_library_page_and_playtime() {
        let page: LibraryPage = serde_json::from_str(
            r#"{"responseMetadata":{"nextCursor":"abc"},"records":[
                {"namespace":"ns","catalogItemId":"cat","appName":"app","sandboxType":"PUBLIC","productId":"p"}]}"#,
        )
        .unwrap();
        assert_eq!(page.records[0].app_name, "app");
        assert_eq!(page.response_metadata.unwrap().next_cursor.as_deref(), Some("abc"));

        let pt: Vec<PlaytimeItem> =
            serde_json::from_str(r#"[{"accountId":"a","artifactId":"Heather","totalTime":46080}]"#).unwrap();
        assert_eq!((pt[0].artifact_id.as_str(), pt[0].total_time), ("Heather", 46080));
    }

    #[test]
    fn catalog_picks_art_and_rejects_addons() {
        let item: CatalogItem = serde_json::from_str(
            r#"{"title":"Celeste","keyImages":[
                {"type":"DieselGameBox","url":"wide.jpg"},{"type":"DieselGameBoxTall","url":"tall.jpg"},
                {"type":"DieselGameBoxLogo","url":"logo.png"}],
               "categories":[{"path":"games"},{"path":"applications"}]}"#,
        )
        .unwrap();
        let c = catalog_entry(&item);
        assert!(c.is_game);
        assert_eq!((c.cover.as_deref(), c.hero.as_deref(), c.logo.as_deref()), (Some("tall.jpg"), Some("wide.jpg"), Some("logo.png")));

        let dlc: CatalogItem = serde_json::from_str(
            r#"{"title":"Soundtrack","categories":[{"path":"addons"}],"mainGameItem":{"id":"x"}}"#,
        )
        .unwrap();
        assert!(!catalog_entry(&dlc).is_game);
    }

    #[test]
    fn extracts_code_from_redirect_json_or_plain_paste() {
        let body = r#"{"warning":"Do not share this code","redirectUrl":"x","authorizationCode":"0123456789abcdef0123456789abcdef","sid":null}"#;
        assert_eq!(code_from_redirect_body(body).as_deref(), Some("0123456789abcdef0123456789abcdef"));
        assert_eq!(clean_code(body).unwrap(), "0123456789abcdef0123456789abcdef");
        assert_eq!(clean_code(" \"0123456789abcdef0123456789abcdef\" ").unwrap(), "0123456789abcdef0123456789abcdef");
        assert!(clean_code("hello").is_err());
    }

    #[test]
    fn login_url_wraps_the_redirect() {
        let u = login_url();
        assert!(u.starts_with("https://www.epicgames.com/id/login?redirectUrl=https%3A%2F%2Fwww.epicgames.com%2Fid%2Fapi%2Fredirect%3FclientId%3D"));
        assert!(u.ends_with("%26responseType%3Dcode"));
    }
}
