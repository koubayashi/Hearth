//! The taskbar jump list (right-click Hearth's taskbar button): the five
//! games played last, as in the tray, and a Console task, as in the rail.
//! Each item runs hearth.exe again with `--play=<id>` or `--console`; that
//! second copy hands its arguments to the running Hearth (single-instance)
//! and exits, so `handle_args` sees them either way.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use hub_core::LibraryQuery;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Listener, Manager};

use crate::{lock, tracking, AppState};

const RECENT: usize = 5;
const CONSOLE_ARG: &str = "--console";
const PLAY_ARG: &str = "--play=";

/// What a jump list click left for the page, which may not be loaded yet
/// when Hearth was in the tray: it takes this on load and on `jump-list`.
#[derive(Default, Serialize, Clone)]
pub struct Request {
    console: bool,
    error: Option<String>,
}

#[derive(Default)]
pub struct Pending(Mutex<Request>);

/// The page takes what's waiting for it (and it's then cleared).
#[tauri::command]
pub fn take_jump_request(pending: tauri::State<Pending>) -> Request {
    std::mem::take(&mut *lock(&pending.0))
}

/// Acts on Hearth's command line, at start or passed on by a second copy.
/// Returns whether the window should open: not when a game was started.
pub fn handle_args(app: &AppHandle, args: &[String]) -> bool {
    if let Some(id) = args.iter().find_map(|a| a.strip_prefix(PLAY_ARG)?.parse::<i64>().ok()) {
        let state = app.state::<AppState>();
        match crate::play(&state, id, None) {
            Ok(()) => return false,
            Err(e) => {
                tracking::log(app, &format!("jump list: couldn't start game {id} ({e})"));
                let title = lock(&state.db).game(id).ok().flatten().map(|g| g.title);
                lock(&app.state::<Pending>().0).error =
                    Some(title.map_or(e.clone(), |t| format!("Couldn't start {t}: {e}")));
            }
        }
    } else if args.iter().any(|a| a == CONSOLE_ARG) {
        lock(&app.state::<Pending>().0).console = true;
    }
    let _ = app.emit("jump-list", ());
    true
}

/// Builds the list now and again whenever the library changes (play ending
/// counts: it moves a game up). Changes come in bursts during a sync, so
/// they're gathered for a moment, and the list is only rewritten when the
/// five games (or the taskbar's theme) actually changed.
pub fn watch(app: &AppHandle) {
    static SCHEDULED: AtomicBool = AtomicBool::new(false);
    let schedule = |app: AppHandle, delay: Duration| {
        if SCHEDULED.swap(true, Ordering::SeqCst) {
            return;
        }
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            SCHEDULED.store(false, Ordering::SeqCst);
            refresh(&app);
        });
    };
    schedule(app.clone(), Duration::ZERO);
    let handle = app.clone();
    app.listen_any("library-changed", move |_| schedule(handle.clone(), Duration::from_secs(3)));
}

#[derive(PartialEq, Clone)]
struct Item {
    id: i64,
    title: String,
    icon: Option<PathBuf>,
}

fn refresh(app: &AppHandle) {
    static SHOWN: Mutex<Option<(Vec<Item>, bool)>> = Mutex::new(None);
    let Some(items) = recent(app) else { return };
    let light = light_taskbar();
    let mut shown = lock(&SHOWN);
    if shown.as_ref().is_some_and(|(i, l)| *i == items && *l == light) {
        return;
    }
    let Some(console_icon) = console_icon(app, light) else { return };
    match write(&items, &console_icon) {
        Ok(()) => *shown = Some((items, light)),
        Err(e) => tracking::log(app, &format!("jump list: couldn't update it ({e})")),
    }
}

/// The same five as the tray: installed, played, not hidden, latest first.
fn recent(app: &AppHandle) -> Option<Vec<Item>> {
    let games = lock(&app.state::<AppState>().db).list_games(&LibraryQuery::default()).ok()?;
    let mut games: Vec<_> = games.into_iter().filter(|g| g.installed && g.last_played.is_some() && !g.hidden).collect();
    games.sort_by_key(|g| std::cmp::Reverse(g.last_played));
    games.truncate(RECENT);
    Some(
        games
            .into_iter()
            .map(|g| {
                let id = g.id;
                let full = lock(&app.state::<AppState>().db).game(id).ok().flatten();
                Item { id, title: g.title, icon: full.and_then(|g| game_icon(&g)) }
            })
            .collect(),
    )
}

/// The game's own program, for its icon: the one Hearth was told to run,
/// else the biggest program in its folder (or one folder down, as in
/// `HITMAN 3\Retail`), leaving out uninstallers and crash reporters.
fn game_icon(game: &hub_core::Game) -> Option<PathBuf> {
    let entry = game.entries.iter().filter(|e| e.installed && !e.offline).max_by_key(|e| e.last_played.unwrap_or(0))?;
    let dir = Path::new(entry.install_dir.as_deref()?);
    if let Some(exe) = entry.launch_exe.as_deref().map(|e| dir.join(e)).filter(|p| p.is_file()) {
        return Some(exe);
    }
    fn look(d: &Path, deeper: &mut Vec<PathBuf>, best: &mut Option<(u64, PathBuf)>) {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let path = e.path();
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                deeper.push(path);
                continue;
            }
            let name = e.file_name().to_string_lossy().to_lowercase();
            if name.ends_with(".exe") && !hub_tracker::is_helper(&name) && best.as_ref().is_none_or(|(s, _)| meta.len() > *s) {
                *best = Some((meta.len(), path));
            }
        }
    }
    let (mut best, mut subdirs) = (None, Vec::new());
    look(dir, &mut subdirs, &mut best);
    if best.is_none() {
        for sub in subdirs {
            look(&sub, &mut Vec::new(), &mut best);
        }
    }
    best.map(|(_, p)| p)
}

/// The rail's Console glyph, drawn for the taskbar's theme
/// (`scripts/console-icon.ps1`). Shell links take an icon by file path, so
/// it's written next to Hearth's data.
fn console_icon(app: &AppHandle, light: bool) -> Option<PathBuf> {
    let (name, bytes): (_, &[u8]) = if light {
        ("console-dark.ico", include_bytes!("../icons/console-dark.ico"))
    } else {
        ("console-light.ico", include_bytes!("../icons/console-light.ico"))
    };
    let path = app.path().app_data_dir().ok()?.join(name);
    if std::fs::read(&path).ok().as_deref() != Some(bytes) {
        std::fs::write(&path, bytes).ok()?;
    }
    Some(path)
}

/// Windows' own theme (the taskbar's), not the apps' one.
#[cfg(windows)]
fn light_taskbar() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut value = 0u32;
    let mut size = 4u32;
    let read = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    read.is_ok() && value == 1
}

#[cfg(windows)]
fn write(items: &[Item], console_icon: &Path) -> windows::core::Result<()> {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::Storage::EnhancedStorage::PKEY_Title;
    use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
    use windows::Win32::System::Variant::VT_LPWSTR;
    use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW, SHStrDupW, ShellLink};

    let exe = std::env::current_exe().map_err(|_| windows::core::Error::empty())?;
    let exe = HSTRING::from(exe.as_os_str());

    let link = |title: &str, args: &str, icon: &Path| -> windows::core::Result<IShellLinkW> {
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(&exe)?;
            link.SetArguments(&HSTRING::from(args))?;
            link.SetIconLocation(&HSTRING::from(icon.as_os_str()), 0)?;
            link.SetDescription(&HSTRING::from(title))?;
            // The shown name is the link's title, set as a property. The
            // value frees its text when dropped (PropVariantClear), so the
            // text comes from COM's allocator.
            let value = PROPVARIANT {
                Anonymous: PROPVARIANT_0 {
                    Anonymous: std::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
                        vt: VT_LPWSTR,
                        Anonymous: PROPVARIANT_0_0_0 { pwszVal: SHStrDupW(&HSTRING::from(title))? },
                        ..Default::default()
                    }),
                },
            };
            let store: IPropertyStore = link.cast()?;
            store.SetValue(&PKEY_Title, &value)?;
            store.Commit()?;
            Ok(link)
        }
    };

    unsafe {
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let result = (|| {
            let list: ICustomDestinationList = CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
            let mut slots = 0u32;
            let removed: IObjectArray = list.BeginList(&mut slots)?;
            // Games the user took off the list ("Remove from this list") must
            // stay off, or Windows refuses the whole list.
            let mut gone = Vec::new();
            for i in 0..removed.GetCount()? {
                if let Ok(l) = removed.GetAt::<IShellLinkW>(i) {
                    let mut buf = [0u16; 64];
                    if l.GetArguments(&mut buf).is_ok() {
                        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                        gone.push(String::from_utf16_lossy(&buf[..end]));
                    }
                }
            }

            let games: IObjectCollection = CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
            let own_icon = PathBuf::from(exe.to_os_string());
            for item in items {
                let args = format!("{PLAY_ARG}{}", item.id);
                if gone.contains(&args) {
                    continue;
                }
                games.AddObject(&link(&item.title, &args, item.icon.as_deref().unwrap_or(&own_icon))?)?;
            }
            if games.GetCount()? > 0 {
                list.AppendCategory(&HSTRING::from("Recent"), &games.cast::<IObjectArray>()?)?;
            }

            // A category of our own, not AddUserTasks: Windows titles that one
            // "Tasks" and it can't be renamed. Unlike tasks, it can be removed
            // from the list, and then must stay off like a game.
            if !gone.iter().any(|a| a == CONSOLE_ARG) {
                let views: IObjectCollection = CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
                views.AddObject(&link("Console", CONSOLE_ARG, console_icon)?)?;
                list.AppendCategory(&HSTRING::from("Views"), &views.cast::<IObjectArray>()?)?;
            }
            list.CommitList()
        })();
        if com.is_ok() {
            CoUninitialize();
        }
        result
    }
}

#[cfg(not(windows))]
fn light_taskbar() -> bool {
    false
}

#[cfg(not(windows))]
fn write(_: &[Item], _: &Path) -> Result<(), String> {
    Ok(())
}
