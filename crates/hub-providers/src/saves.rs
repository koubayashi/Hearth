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
/// Real save paths have one or two (`{{p|uid}}`, `*.sav`).
const MAX_WILDCARDS: usize = 3;

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
        let text = match self.wikitext(&page).await? {
            Some(t) => t,
            // A folder's name often drops the title's punctuation
            // ("Clair Obscur Expedition 33"): find the page by search instead.
            None => match self.search_page(&page).await? {
                Some(found) => self.wikitext(&found).await?.unwrap_or_default(),
                None => String::new(),
            },
        };
        Ok(windows_save_paths(&text))
    }

    /// A page's wikitext, or None when there's no such page.
    async fn wikitext(&self, page: &str) -> anyhow::Result<Option<String>> {
        let body: serde_json::Value = self
            .http
            .get(API)
            .query(&[
                ("action", "parse"),
                ("page", page),
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
        Ok(body.pointer("/parse/wikitext").and_then(|v| v.as_str()).map(str::to_string))
    }

    /// The page whose title is `title` give or take punctuation and case.
    async fn search_page(&self, title: &str) -> anyhow::Result<Option<String>> {
        let body: serde_json::Value = self
            .http
            .get(API)
            .query(&[("action", "query"), ("list", "search"), ("srsearch", title), ("srlimit", "5"), ("format", "json")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let want = loose(title);
        Ok(body
            .pointer("/query/search")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|r| r.get("title")?.as_str())
            .find(|t| loose(t) == want)
            .map(str::to_string))
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

/// Lowercased letters and digits only, for matching titles loosely.
fn loose(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// The PC entries of `{{Game data/saves|<platform>|path|path…}}`: the
/// "Windows" row, and the per-store rows (Steam, Epic Games Launcher, GOG.com,
/// Microsoft Store, …) newer pages use instead. Mac and Linux rows are skipped.
pub fn windows_save_paths(wikitext: &str) -> Vec<String> {
    const NOT_WINDOWS: &[&str] = &["os x", "macos", "mac os", "linux"];
    let mut out: Vec<String> = Vec::new();
    for line in wikitext.lines() {
        let Some(rest) = line.trim().strip_prefix("{{Game data/saves|") else { continue };
        let Some((platform, rest)) = rest.split_once('|') else { continue };
        let platform = platform.to_lowercase();
        if NOT_WINDOWS.iter().any(|n| platform.contains(n)) {
            continue;
        }
        for part in split_top_level(rest.trim_end_matches("}}")) {
            // Drop wiki notes like ''(Re-Release 2020)'' and references.
            let p = part.split("''").next().unwrap_or("").split("<ref").next().unwrap_or("").trim();
            if !p.is_empty() && !out.iter().any(|o| o == p) {
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
            "userprofile\\appdata\\locallow" => places.user_profile.join(r"AppData\LocalLow").to_string_lossy().into_owned(),
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
    // The wiki is publicly editable: only a plain path on a local drive
    // ("C:\..."), never one climbing out with "..", a network share or a
    // path relative to wherever Hearth happens to run.
    let local = out.as_bytes().get(..3).is_some_and(|b| b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\');
    if !local || out.split('\\').any(|p| p.trim() == "..") {
        return None;
    }
    // A path that resolves to a bare known folder would date every game.
    let bare = [&places.appdata, &places.local_appdata, &places.user_profile, &places.documents, &places.program_data]
        .iter()
        .any(|p| out.eq_ignore_ascii_case(&p.to_string_lossy()));
    (!bare && out.len() > 3).then_some(out)
}

/// Newest modification time (unix seconds) among files matching a resolved
/// path, where `*` matches any one folder or file-name part.
pub fn newest_file(pattern: &str) -> Option<i64> {
    let mut budget = MAX_ENTRIES;
    matching(pattern).iter().filter_map(|r| newest_under(r, 0, &mut budget)).max()
}

/// The files and folders a resolved path names, `*` expanded.
fn matching(pattern: &str) -> Vec<PathBuf> {
    let parts: Vec<&str> = pattern.split('\\').filter(|p| !p.is_empty()).collect();
    let Some(first) = parts.first() else { return Vec::new() };
    // Wildcards multiply: a wiki path of nothing but `*` folders would
    // otherwise walk the whole profile on every sync.
    if parts.iter().filter(|p| p.contains('*')).count() > MAX_WILDCARDS {
        return Vec::new();
    }
    let mut budget = MAX_ENTRIES;
    let mut roots = vec![PathBuf::from(format!("{first}\\"))];
    for (i, part) in parts.iter().enumerate().skip(1) {
        let last = i == parts.len() - 1;
        let mut next = Vec::new();
        for r in &roots {
            if part.contains('*') {
                let Ok(read) = std::fs::read_dir(r) else { continue };
                for e in read.flatten() {
                    if budget == 0 {
                        break;
                    }
                    budget -= 1;
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
            break;
        }
    }
    roots
}

/// The game's own play timer, in seconds, from Unreal Engine saves (`GVAS`
/// files) under a resolved path: the largest `TimePlayed`-style value, so
/// several save slots never add up. None when no save has one.
pub fn played_secs(pattern: &str) -> Option<i64> {
    let mut files = Vec::new();
    for root in matching(pattern) {
        if root.is_file() {
            files.push(root);
        } else if let Ok(read) = std::fs::read_dir(&root) {
            files.extend(read.flatten().map(|e| e.path()).filter(|p| p.is_file()));
        }
    }
    files
        .iter()
        .filter(|f| f.extension().is_some_and(|e| e.eq_ignore_ascii_case("sav")))
        .filter(|f| std::fs::metadata(f).is_ok_and(|m| m.len() <= MAX_SAVE_BYTES))
        .filter_map(|f| std::fs::read(f).ok())
        .filter_map(|bytes| gvas_played_secs(&bytes))
        .max()
}

const MAX_SAVE_BYTES: u64 = 32 * 1024 * 1024;
/// Property names games use for their play timer, in seconds.
const TIMER_NAMES: &[&str] = &["TimePlayed", "PlayTime", "Playtime", "PlayedTime", "TotalPlayTime", "TotalTimePlayed"];

/// Finds a play-timer property in an Unreal save: its name (exactly, or with
/// a Blueprint's `_6_<guid>` suffix), then its type, then 9 bytes of tag (size
/// and flags, in both UE4's and UE5's layouts) before the value.
fn gvas_played_secs(bytes: &[u8]) -> Option<i64> {
    if !bytes.starts_with(b"GVAS") {
        return None;
    }
    let mut best: Option<f64> = None;
    for name in TIMER_NAMES {
        let mut from = 0;
        while let Some(at) = find(&bytes[from..], name.as_bytes()).map(|i| i + from) {
            from = at + 1;
            let end = at + name.len();
            // The name has to end here (or at a Blueprint suffix), so
            // "PlayTimeLimit" and the like don't count.
            if !matches!(bytes.get(end), Some(0 | b'_')) || !matches!(bytes.get(at.wrapping_sub(1)), Some(&n) if n < 0x20) {
                continue;
            }
            let Some(name_end) = bytes[end..].iter().position(|&b| b == 0).map(|i| end + i + 1) else { continue };
            // FString: int32 length, then the text.
            let Some(len) = bytes.get(name_end..name_end + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize) else { continue };
            let Some(kind) = bytes.get(name_end + 4..name_end + 4 + len) else { continue };
            let value = name_end + 4 + len + 9;
            let secs = match kind {
                b"DoubleProperty\0" => bytes.get(value..value + 8).map(|b| f64::from_le_bytes(b.try_into().unwrap())),
                b"FloatProperty\0" => bytes.get(value..value + 4).map(|b| f64::from(f32::from_le_bytes(b.try_into().unwrap()))),
                b"IntProperty\0" => bytes.get(value..value + 4).map(|b| f64::from(i32::from_le_bytes(b.try_into().unwrap()))),
                _ => None,
            };
            // A minute up to 20,000 hours; anything else isn't seconds played.
            if let Some(s) = secs.filter(|s| (60.0..72_000_000.0).contains(s)) {
                best = Some(best.map_or(s, |b| b.max(s)));
            }
        }
    }
    best.map(|s| s as i64)
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
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
    if pieces.len() == 1 {
        return p == n;
    }
    // The first piece starts the name and the last ends it ("*.sav" matches
    // "a.sav.sav"); the ones between appear in order in what's left.
    let (first, last) = (pieces[0], pieces[pieces.len() - 1]);
    if n.len() < first.len() + last.len() || !n.starts_with(first) || !n.ends_with(last) {
        return false;
    }
    let mut middle = &n[first.len()..n.len() - last.len()];
    for piece in &pieces[1..pieces.len() - 1] {
        match middle.find(piece) {
            Some(at) => middle = &middle[at + piece.len()..],
            None => return false,
        }
    }
    true
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

    #[test]
    fn reads_per_store_save_rows_once() {
        // Clair Obscur: Expedition 33's page, which has no "Windows" row.
        let text = "{{Game data/saves|Epic Games Launcher|{{p|localappdata}}\\Sandfall\\Saved\\SaveGames\\{{p|uid}}\\*.sav}}\n{{Game data/saves|Steam|{{p|localappdata}}\\Sandfall\\Saved\\SaveGames\\{{p|uid}}\\*.sav}}\n{{Game data/saves|OS X|~/Library/x}}";
        assert_eq!(windows_save_paths(text), ["{{p|localappdata}}\\Sandfall\\Saved\\SaveGames\\{{p|uid}}\\*.sav"]);
    }

    /// An Unreal save property: name, type, 9 bytes of tag, value.
    fn property(name: &str, kind: &str, value: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for s in [name, kind] {
            out.extend((s.len() as u32 + 1).to_le_bytes());
            out.extend(s.as_bytes());
            out.push(0);
        }
        out.extend([0, 0, 0, 0, value.len() as u8, 0, 0, 0, 0]);
        out.extend(value);
        out
    }

    #[test]
    fn reads_an_unreal_save_timer() {
        let mut save = b"GVAS\x03\0\0\0".to_vec();
        save.extend(property("PlayTimeLimit", "DoubleProperty", &99_999.0f64.to_le_bytes()));
        save.extend(property("TimePlayed", "DoubleProperty", &20_746.4f64.to_le_bytes()));
        save.extend(property("TimePlayed_6_37712C2447FFCEA1BCF1B6AAD59EA130", "DoubleProperty", &100.0f64.to_le_bytes()));
        assert_eq!(gvas_played_secs(&save), Some(20_746), "the largest timer, not a look-alike name");

        let mut ints = b"GVAS".to_vec();
        ints.extend(property("PlayTime", "IntProperty", &3_600i32.to_le_bytes()));
        assert_eq!(gvas_played_secs(&ints), Some(3_600));

        assert_eq!(gvas_played_secs(&save[4..]), None, "not a GVAS file");
        let mut zero = b"GVAS".to_vec();
        zero.extend(property("TimePlayed", "DoubleProperty", &0.0f64.to_le_bytes()));
        assert_eq!(gvas_played_secs(&zero), None, "an unplayed slot");
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
        assert_eq!(
            resolve(r"{{P|userprofile\appdata\locallow}}\AttritoM7 Productions\Blackwood\blackwood.sav", &p, None).as_deref(),
            Some(r"C:\Users\me\AppData\LocalLow\AttritoM7 Productions\Blackwood\blackwood.sav")
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
        assert!(wildcard("*.sav", "slot.sav.sav"), "the last piece ends the name");
        assert!(wildcard("a*b*c", "aXbYbc"));
        assert!(!wildcard("ab*ba", "aba"), "the ends can't overlap");
        assert!(!wildcard("save", "save1"));
    }

    #[test]
    fn wiki_paths_stay_on_local_drives() {
        let p = places(Path::new(r"C:\Users\me"));
        assert_eq!(resolve(r"{{p|appdata}}\..\..\..\Windows", &p, None), None);
        assert_eq!(resolve(r"\\attacker.example\share\saves", &p, None), None);
        assert_eq!(resolve(r"Some Game\saves", &p, None), None);
        assert!(matching(r"C:\*\*\*\*\*").is_empty(), "too many wildcards");
    }
}
