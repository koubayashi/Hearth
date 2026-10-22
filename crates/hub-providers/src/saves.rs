//! "Last played" from save files. Almost every game writes its save when you
//! play, so the newest file in its save folder dates the last session, for
//! any store and however long ago. Save locations come from PCGamingWiki
//! (the same data save-backup tools like Ludusavi use); no key needed.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::Context;

const API: &str = "https://www.pcgamingwiki.com/w/api.php";

/// Don't wander forever through a big folder.
const MAX_ENTRIES: usize = 20_000;
const MAX_DEPTH: usize = 6;

pub struct Saves {
    http: reqwest::Client,
}

impl Default for Saves {
    fn default() -> Self {
        Self {
            http: reqwest::Client::builder().user_agent("Hearth/0.1 (game library)").build().expect("http client"),
        }
    }
}

impl Saves {
    /// Windows save locations for a game, as PCGamingWiki writes them
    /// (with `{{p|…}}` placeholders). Found by Steam app id when known, else
    /// by page title.
    pub async fn locations(&self, title: &str, steam_app: Option<u64>) -> anyhow::Result<Vec<String>> {
        let page = match steam_app {
            Some(id) => self.page_for_steam_app(id).await?,
            None => None,
        };
        let page = page.unwrap_or_else(|| title.replace(['™', '®'], "").trim().to_string());
        let body: serde_json::Value = self
            .http
            .get(API)
            .query(&[
                ("action", "parse"),
                ("page", page.as_str()),
                ("prop", "wikitext"),
                ("redirects", "1"),
                ("format", "json"),
                ("formatversion", "2"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("reading PCGamingWiki")?;
        let text = body.pointer("/parse/wikitext").and_then(|v| v.as_str()).unwrap_or_default();
        Ok(windows_save_paths(text))
    }

    async fn page_for_steam_app(&self, id: u64) -> anyhow::Result<Option<String>> {
        let filter = format!("Steam_AppID HOLDS \"{id}\"");
        let body: serde_json::Value = self
            .http
            .get(API)
            .query(&[
                ("action", "cargoquery"),
                ("tables", "Infobox_game"),
                ("fields", "_pageName=page"),
                ("where", filter.as_str()),
                ("format", "json"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(body
            .pointer("/cargoquery/0/title/page")
            .and_then(|v| v.as_str())
            .map(str::to_string))
    }
}

/// The Windows entries of `{{Game data/saves|Windows|path|path…}}`.
pub fn windows_save_paths(wikitext: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in wikitext.lines() {
        let Some(rest) = line.trim().strip_prefix("{{Game data/saves|Windows|") else { continue };
        for part in split_top_level(rest.trim_end_matches("}}")) {
            // Drop wiki notes like ''(Re-Release 2020)'' and references.
            let p = part.split("''").next().unwrap_or("").split("<ref").next().unwrap_or("").trim();
            if !p.is_empty() {
                out.push(p.to_string());
            }
        }
    }
    out
}

/// Splits on `|` outside `{{ }}`, so placeholders stay whole.
fn split_top_level(s: &str) -> Vec<String> {
    let (mut parts, mut cur, mut depth) = (Vec::new(), String::new(), 0i32);
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '{' && chars.get(i + 1) == Some(&'{') {
            depth += 1;
            cur.push_str("{{");
            i += 2;
            continue;
        }
        if c == '}' && chars.get(i + 1) == Some(&'}') {
            depth -= 1;
            cur.push_str("}}");
            i += 2;
            continue;
        }
        if c == '|' && depth == 0 {
            parts.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
        i += 1;
    }
    parts.push(cur);
    parts
}

/// Where Windows keeps the folders PCGamingWiki's placeholders stand for.
pub struct Places {
    pub appdata: PathBuf,
    pub local_appdata: PathBuf,
    pub user_profile: PathBuf,
    /// The real Documents folder (often redirected to OneDrive).
    pub documents: PathBuf,
    pub saved_games: PathBuf,
    pub program_data: PathBuf,
    pub public: PathBuf,
}

impl Places {
    pub fn current() -> Option<Self> {
        let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
        let user_profile = env("USERPROFILE")?;
        Some(Self {
            appdata: env("APPDATA")?,
            local_appdata: env("LOCALAPPDATA")?,
            documents: shell_folder("Personal").unwrap_or_else(|| user_profile.join("Documents")),
            saved_games: shell_folder("{4C5C32FF-BB9D-43B0-B5B4-2D72E54EAAA4}").unwrap_or_else(|| user_profile.join("Saved Games")),
            program_data: env("PROGRAMDATA").unwrap_or_else(|| PathBuf::from(r"C:\ProgramData")),
            public: env("PUBLIC").unwrap_or_else(|| PathBuf::from(r"C:\Users\Public")),
            user_profile,
        })
    }
}

#[cfg(windows)]
fn shell_folder(name: &str) -> Option<PathBuf> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let v: String = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders")
        .ok()?
        .get_value(name)
        .ok()?;
    Some(PathBuf::from(expand_env(&v)))
}

#[cfg(not(windows))]
fn shell_folder(_name: &str) -> Option<PathBuf> {
    None
}

fn expand_env(s: &str) -> String {
    let mut out = s.to_string();
    while let Some(start) = out.find('%') {
        let Some(len) = out[start + 1..].find('%') else { break };
        let name = &out[start + 1..start + 1 + len];
        let value = std::env::var(name).unwrap_or_default();
        out.replace_range(start..start + len + 2, &value);
    }
    out
}

/// A PCGamingWiki path made concrete, with `*` where it holds a user id or a
/// wildcard. None when it uses a place Hearth can't resolve (registry, …).
pub fn resolve(path: &str, places: &Places, install_dir: Option<&Path>) -> Option<String> {
    let mut out = String::new();
    let mut rest = path;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let end = rest[start..].find("}}")? + start;
        let inner = rest[start + 2..end].to_lowercase();
        let key = inner.strip_prefix("p|")?;
        let place = match key {
            "appdata" => places.appdata.to_string_lossy().into_owned(),
            "localappdata" => places.local_appdata.to_string_lossy().into_owned(),
            "userprofile" => places.user_profile.to_string_lossy().into_owned(),
            "userprofile\\documents" => places.documents.to_string_lossy().into_owned(),
            "userprofile\\saved games" => places.saved_games.to_string_lossy().into_owned(),
            "programdata" => places.program_data.to_string_lossy().into_owned(),
            "public" => places.public.to_string_lossy().into_owned(),
            "game" => install_dir?.to_string_lossy().into_owned(),
            "uid" | "username" | "userid" => "*".to_string(),
            _ => return None,
        };
        out.push_str(&place);
        rest = &rest[end + 2..];
    }
    out.push_str(rest);
    let out = out.replace('/', "\\").trim().trim_end_matches('\\').to_string();
    // A path that resolves to a bare known folder would date every game.
    let bare = [&places.appdata, &places.local_appdata, &places.user_profile, &places.documents, &places.program_data]
        .iter()
        .any(|p| out.eq_ignore_ascii_case(&p.to_string_lossy()));
    (!bare && out.len() > 3).then_some(out)
}

/// Newest modification time (unix seconds) among files matching a resolved
/// path, where `*` matches any one folder or file-name part.
pub fn newest_file(pattern: &str) -> Option<i64> {
    let parts: Vec<&str> = pattern.split('\\').filter(|p| !p.is_empty()).collect();
    let first = parts.first()?;
    let mut roots = vec![PathBuf::from(format!("{first}\\"))];
    for (i, part) in parts.iter().enumerate().skip(1) {
        let last = i == parts.len() - 1;
        let mut next = Vec::new();
        for r in &roots {
            if part.contains('*') {
                let Ok(read) = std::fs::read_dir(r) else { continue };
                for e in read.flatten() {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if wildcard(part, &name) && (last || e.path().is_dir()) {
                        next.push(e.path());
                    }
                }
            } else {
                let p = r.join(part);
                if p.exists() {
                    next.push(p);
                }
            }
        }
        roots = next;
        if roots.is_empty() {
            return None;
        }
    }
    let mut budget = MAX_ENTRIES;
    roots.iter().filter_map(|r| newest_under(r, 0, &mut budget)).max()
}

fn newest_under(p: &Path, depth: usize, budget: &mut usize) -> Option<i64> {
    let meta = std::fs::metadata(p).ok()?;
    if meta.is_file() {
        return modified(&meta);
    }
    if depth >= MAX_DEPTH || *budget == 0 {
        return None;
    }
    let mut best = None;
    for e in std::fs::read_dir(p).ok()?.flatten() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        best = best.max(newest_under(&e.path(), depth + 1, budget));
    }
    // An emptied save folder (saves moved to the cloud or deleted) still
    // dates the last time the game wrote there.
    best.or_else(|| modified(&meta))
}

fn modified(meta: &std::fs::Metadata) -> Option<i64> {
    let t = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(t).ok()
}

/// `*` wildcard matching, case-insensitive like Windows.
fn wildcard(pattern: &str, name: &str) -> bool {
    let (p, n) = (pattern.to_lowercase(), name.to_lowercase());
    let pieces: Vec<&str> = p.split('*').collect();
    let mut pos = 0;
    for (i, piece) in pieces.iter().enumerate() {
        if piece.is_empty() {
            continue;
        }
        match n[pos..].find(piece) {
            Some(at) if i > 0 || at == 0 => pos += at + piece.len(),
            _ => return false,
        }
    }
    pieces.last().is_some_and(|l| l.is_empty()) || pos == n.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_windows_save_paths_from_wikitext() {
        let text = "{{Game data|\n{{Game data/config|Windows|{{p|appdata}}\\X\\}}\n{{Game data/saves|Windows|{{p|userprofile\\Documents}}\\Freedom Fighters\\Profiles\\*.dat|{{P|appdata}}\\IO Interactive\\Freedom Fighters\\ ''(Re-Release 2020)''}}\n{{Game data/saves|Steam Play (Linux)|x}}\n}}";
        assert_eq!(
            windows_save_paths(text),
            ["{{p|userprofile\\Documents}}\\Freedom Fighters\\Profiles\\*.dat", "{{P|appdata}}\\IO Interactive\\Freedom Fighters\\"]
        );
    }

    fn places(root: &Path) -> Places {
        Places {
            appdata: root.join("Roaming"),
            local_appdata: root.join("Local"),
            user_profile: root.to_path_buf(),
            documents: root.join("OneDrive").join("Documents"),
            saved_games: root.join("Saved Games"),
            program_data: root.join("ProgramData"),
            public: root.join("Public"),
        }
    }

    #[test]
    fn resolves_placeholders_and_skips_unknown_ones() {
        let p = places(Path::new(r"C:\Users\me"));
        assert_eq!(
            resolve(r"{{P|userprofile\Documents}}\Rockstar Games\GTA V\Profiles\{{P|uid}}", &p, None).as_deref(),
            Some(r"C:\Users\me\OneDrive\Documents\Rockstar Games\GTA V\Profiles\*")
        );
        assert_eq!(resolve(r"{{p|hkcu}}\Software\X", &p, None), None, "registry isn't a folder");
        assert_eq!(resolve(r"{{p|game}}\save", &p, None), None, "needs an install folder");
        assert_eq!(resolve(r"{{p|game}}\save", &p, Some(Path::new(r"D:\Games\X"))).as_deref(), Some(r"D:\Games\X\save"));
        assert_eq!(resolve(r"{{p|appdata}}\", &p, None), None, "a bare known folder would date everything");
    }

    #[test]
    fn finds_the_newest_save_through_wildcards() {
        let root = std::env::temp_dir().join(format!("hearth-saves-{}", std::process::id()));
        let profile = root.join("Rockstar Games").join("GTA V").join("Profiles").join("0D8577CB");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(profile.join("SGTA50000"), b"save").unwrap();
        std::fs::write(root.join("unrelated.txt"), b"x").unwrap();

        let pattern = format!(r"{}\Rockstar Games\GTA V\Profiles\*", root.display());
        let t = newest_file(&pattern).expect("found the save");
        let now = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        assert!((now - t).abs() < 120);
        assert_eq!(newest_file(&format!(r"{}\Rockstar Games\GTA V\Profiles\*.dat", root.display())), None);
        assert!(newest_file(&format!(r"{}\Rockstar Games\GTA V\Profiles\*\SGTA*", root.display())).is_some());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn wildcards_match_like_windows() {
        assert!(wildcard("*.dat", "FREEDOM FIGHTER.dat"));
        assert!(wildcard("SGTA*", "sgta50000"));
        assert!(!wildcard("*.dat", "keyboard.cfg"));
        assert!(wildcard("*", "anything"));
    }
}
