//! Games that live in plain folders (DRM-free installs, copies from other
//! PCs, games from launchers Hearth doesn't know). The user picks one or more
//! game folders; each game inside is found, whether it sits directly in the
//! folder or in a sub-folder, and its main executable is chosen.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use hub_core::{GameEntry, StoreId};
use hub_launch::LaunchAction;

use crate::LibraryProvider;

/// How far below a chosen folder to look for games ("Games\AC\Assassin's Creed" is 2).
const MAX_DEPTH: usize = 3;

/// Executables that ship with games but aren't the game.
const NOT_GAME_EXES: &[&str] = &[
    "unins", "uninstall", "setup", "install", "redist", "vcredist", "vc_redist", "dxsetup",
    "dxwebsetup", "dotnet", "directx", "crash", "bugsplat", "unitycrashhandler", "ueprereq",
    "prereq", "quicksfv", "register", "detection", "testapp", "benchmark", "config", "settings",
    "easyanticheat", "battleye", "beservice", "eac", "updater", "patch", "report", "helper",
    "cefprocess", "webhelper", "touchup", "activation", "dxdiag", "vcrun", "oalinst", "physx",
];

/// Folders inside a game that hold tools, never the game itself.
const NOT_GAME_DIRS: &[&str] = &[
    "_redist", "redist", "_commonredist", "commonredist", "support", "detection", "register",
    "directx", "dotnet", "vcredist", "__installer", "installer", "_installer", "easyanticheat",
    "battleye", "crashreporter",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundGame {
    pub title: String,
    pub dir: PathBuf,
    pub exe: PathBuf,
}

pub struct Folders {
    roots: Vec<PathBuf>,
    /// Install folders other stores already own; never offered twice.
    skip: Vec<PathBuf>,
}

impl Folders {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self { roots, skip: Vec::new() }
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub fn set_roots(&mut self, roots: Vec<PathBuf>) {
        self.roots = roots;
    }

    pub fn set_skip(&mut self, dirs: Vec<PathBuf>) {
        self.skip = dirs;
    }

    pub fn scan(&self) -> Vec<FoundGame> {
        let mut out = Vec::new();
        for root in &self.roots {
            let Ok(children) = std::fs::read_dir(root) else { continue };
            for child in sorted_dirs(children) {
                self.visit(&child, 1, &mut out);
            }
        }
        out
    }

    fn visit(&self, dir: &Path, depth: usize, out: &mut Vec<FoundGame>) {
        if is_store_library(dir) || self.skip.iter().any(|s| same(s, dir)) {
            return;
        }
        if let Some(exe) = main_exe(dir) {
            out.push(FoundGame { title: title_for(dir, &exe), dir: dir.to_path_buf(), exe });
            return;
        }
        // No game here: treat it as a folder of games, like "Games\AC\".
        if depth < MAX_DEPTH {
            if let Ok(children) = std::fs::read_dir(dir) {
                for child in sorted_dirs(children) {
                    self.visit(&child, depth + 1, out);
                }
            }
        }
    }
}

fn sorted_dirs(read: std::fs::ReadDir) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = read
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .filter(|p| !lower_name(p).starts_with('.') && !is_tool_dir(p))
        .collect();
    dirs.sort();
    dirs
}

fn lower_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn stem(p: &Path) -> String {
    p.file_stem().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn is_tool_dir(p: &Path) -> bool {
    NOT_GAME_DIRS.contains(&lower_name(p).as_str())
}

fn same(a: &Path, b: &Path) -> bool {
    let n = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    n(a) == n(b)
}

/// A Steam library or the Epic Games folder: those stores list their games.
fn is_store_library(dir: &Path) -> bool {
    dir.join("steamapps").is_dir() || lower_name(dir) == "steamapps"
}

fn exes_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<PathBuf> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")))
        .filter(|p| {
            let s = stem(p);
            !NOT_GAME_EXES.iter().any(|bad| s.contains(bad))
        })
        .collect();
    v.sort();
    v
}

/// The executable to launch for a game folder, or None if it isn't one.
///
/// Engine layouts are recognised first, then the folder's own executables.
pub fn main_exe(dir: &Path) -> Option<PathBuf> {
    let top = exes_in(dir);

    // Unity: Game.exe sits next to Game_Data.
    if let Some(exe) = top.iter().find(|e| dir.join(format!("{}_Data", file_stem(e))).is_dir()) {
        return Some(exe.clone());
    }

    // Unreal: a small starter exe on top, the real one in <Project>/Binaries/Win64.
    let unreal = unreal_shipping_exe(dir);
    if unreal.is_some() {
        return top.first().cloned().or(unreal);
    }

    match top.len() {
        0 => None,
        1 => top.into_iter().next(),
        _ => {
            // Several: a launcher if the game ships one (it picks DX mode etc.),
            // else the one named most like the folder, else the biggest.
            if let Some(l) = top.iter().find(|e| stem(e).contains("launcher")) {
                return Some(l.clone());
            }
            let folder = squash(&lower_name(dir));
            let size = |e: &PathBuf| e.metadata().map(|m| m.len()).unwrap_or(0);
            let named: Vec<&PathBuf> = top
                .iter()
                .filter(|e| {
                    let s = squash(&stem(e));
                    !s.is_empty() && (folder.contains(&s) || s.contains(&folder))
                })
                .collect();
            if let Some(best) = named.into_iter().max_by_key(|e| size(e)) {
                return Some(best.clone());
            }
            top.into_iter().max_by_key(size)
        }
    }
}

fn file_stem(p: &Path) -> String {
    p.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect()
}

fn unreal_shipping_exe(dir: &Path) -> Option<PathBuf> {
    let read = std::fs::read_dir(dir).ok()?;
    for project in read.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        let bin = project.join("Binaries").join("Win64");
        if let Ok(files) = std::fs::read_dir(&bin) {
            if let Some(exe) = files
                .flatten()
                .map(|e| e.path())
                .find(|p| stem(p).ends_with("-shipping") && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")))
            {
                return Some(exe);
            }
        }
    }
    None
}

/// The folder's name, unless it's SHOUTING and the exe has a nicer spelling.
fn title_for(dir: &Path, exe: &Path) -> String {
    let folder = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let exe_name = file_stem(exe);
    let shouting = folder.chars().any(|c| c.is_alphabetic()) && !folder.chars().any(|c| c.is_lowercase());
    if shouting && exe_name.eq_ignore_ascii_case(&folder) {
        exe_name
    } else {
        folder
    }
}

#[async_trait]
impl LibraryProvider for std::sync::Mutex<Folders> {
    fn id(&self) -> StoreId {
        StoreId::Manual
    }

    async fn installed(&self) -> anyhow::Result<Vec<GameEntry>> {
        let found = self.lock().unwrap_or_else(|e| e.into_inner()).scan();
        Ok(found
            .into_iter()
            .map(|g| GameEntry {
                store: StoreId::Manual,
                // The exe path identifies the game: stable across rescans.
                store_game_id: g.exe.to_string_lossy().into_owned(),
                title: g.title,
                installed: true,
                install_dir: Some(g.dir.to_string_lossy().into_owned()),
                store_playtime_secs: None,
                store_last_played: None,
            })
            .collect())
    }

    fn launch(&self, store_game_id: &str) -> LaunchAction {
        let exe = PathBuf::from(store_game_id);
        LaunchAction::Process { working_dir: exe.parent().map(Path::to_path_buf), exe, args: Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Tree(PathBuf);
    impl Tree {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!("hearth-folders-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            Tree(root)
        }
        fn file(&self, rel: &str, bytes: usize) -> &Self {
            let p = self.0.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, vec![0u8; bytes]).unwrap();
            self
        }
        fn dir(&self, rel: &str) -> &Self {
            fs::create_dir_all(self.0.join(rel)).unwrap();
            self
        }
    }
    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn names(found: &[FoundGame]) -> Vec<(String, String)> {
        found
            .iter()
            .map(|g| (g.title.clone(), g.exe.file_name().unwrap().to_string_lossy().into_owned()))
            .collect()
    }

    #[test]
    fn finds_games_in_direct_and_nested_folders_like_d_games() {
        let t = Tree::new("layout");
        // A game inside a sub-folder, with several exes.
        t.file("AC/Assassin's Creed/AssassinsCreed_Dx9.exe", 900)
            .file("AC/Assassin's Creed/AssassinsCreed_Dx10.exe", 800)
            .file("AC/Assassin's Creed/AssassinsCreed_Launcher.exe", 100)
            .file("AC/Assassin's Creed/testapp.exe", 50)
            .file("AC/Assassin's Creed/Detection/Detection.exe", 50)
            .file("AC/Assassin's Creed/Register/RegistrationReminder.exe", 50);
        // Unity, folder name in capitals.
        t.file("BLACKWOOD/Blackwood.exe", 100)
            .dir("BLACKWOOD/Blackwood_Data")
            .file("BLACKWOOD/UnityCrashHandler64.exe", 100)
            .file("BLACKWOOD/unins000.exe", 100)
            .file("BLACKWOOD/_Redist/dxwebsetup.exe", 100);
        // Unreal: starter exe on top, shipping build deep inside.
        t.file("Clair Obscur Expedition 33/Expedition33_Steam.exe", 100)
            .file("Clair Obscur Expedition 33/Sandfall/Binaries/Win64/SandFall-Win64-Shipping.exe", 900)
            .file("Clair Obscur Expedition 33/Engine/Extras/Redist/en-us/UEPrereqSetup_x64.exe", 500);
        // Store libraries are left to their stores.
        t.file("SteamLibrary/steamapps/common/HITMAN 3/HITMAN3.exe", 100);
        t.file("Epic Games/RedDeadRedemption2/PlayRDR2.exe", 100);
        // A folder with no game at all.
        t.file("Saves/readme.txt", 10);

        let mut f = Folders::new(vec![t.0.clone()]);
        f.set_skip(vec![t.0.join("Epic Games").join("RedDeadRedemption2")]);
        assert_eq!(
            names(&f.scan()),
            [
                ("Assassin's Creed".into(), "AssassinsCreed_Launcher.exe".into()),
                ("Blackwood".into(), "Blackwood.exe".into()),
                ("Clair Obscur Expedition 33".into(), "Expedition33_Steam.exe".into()),
            ]
        );
    }

    #[test]
    fn picks_exe_named_like_the_folder_then_the_biggest() {
        let t = Tree::new("pick");
        t.file("Hollow Knight/hollow_knight.exe", 10).file("Hollow Knight/tool.exe", 99);
        assert_eq!(main_exe(&t.0.join("Hollow Knight")).unwrap().file_name().unwrap(), "hollow_knight.exe");
        t.file("Quake/glquake.exe", 10).file("Quake/winquake.exe", 99);
        assert_eq!(main_exe(&t.0.join("Quake")).unwrap().file_name().unwrap(), "winquake.exe");
    }
}
