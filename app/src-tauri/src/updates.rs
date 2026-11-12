//! Updates from GitHub Releases. Installed builds check shortly after start
//! and twice a day, download a newer Hearth in the background, and install it
//! only when the user says so (Settings or the tray), so a game being timed
//! is never cut off. Updates are signed; the updater refuses unsigned ones.
//!
//! Every change of state (checking, downloading, ready, a failed check) goes
//! to the interface as `update-status`, so the rail, Settings and the console
//! show the same thing. When Hearth last checked and the version the user
//! last saw are kept in `updates.json`, so they survive a restart: the second
//! is how Hearth knows to say "Updated to 0.x" once after an update.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Error as UpdateError, Update, UpdaterExt};

use crate::{lock, now, tracking, tray, CmdResult};

const FIRST_CHECK: Duration = Duration::from_secs(60);
const CHECK_EVERY: Duration = Duration::from_secs(12 * 3600);
/// Download progress goes to the interface at most this often.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

/// The release notes of every version, built in: what changed shows without
/// internet and always matches the version installed.
const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

#[derive(Default)]
pub struct Updates {
    /// A downloaded update, waiting for the user to restart into it.
    ready: Mutex<Option<(Update, Vec<u8>)>>,
    status: Mutex<Status>,
}

#[derive(Serialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    Downloading,
    Ready,
    Installing,
}

/// What the interface shows about updates.
#[derive(Serialize, Clone, Default)]
pub struct Status {
    phase: Phase,
    /// The version being downloaded or waiting to install.
    version: Option<String>,
    /// Its release notes (the version's CHANGELOG.md section).
    notes: Option<String>,
    /// Bytes downloaded so far, and the total when the server gives one.
    done: u64,
    total: Option<u64>,
    /// When a check last reached GitHub (unix seconds).
    last_checked: Option<i64>,
    /// Why the last check or install failed, in plain words.
    error: Option<String>,
    /// Hearth was updated to this version and the user hasn't seen it yet.
    updated_to: Option<String>,
    /// A development build, which never checks.
    dev: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct Saved {
    last_checked: Option<i64>,
    last_error: Option<String>,
    /// The version the user has been told about (or was running) last.
    seen_version: Option<String>,
}

fn saved_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("updates.json"))
}

fn load_saved(app: &AppHandle) -> Option<Saved> {
    let text = std::fs::read_to_string(saved_path(app)?).ok()?;
    serde_json::from_str(&text).ok()
}

fn save(app: &AppHandle, status: &Status) {
    let Some(path) = saved_path(app) else { return };
    let current = app.package_info().version.to_string();
    let saved = Saved {
        last_checked: status.last_checked,
        last_error: status.error.clone(),
        // Until the user has seen "Updated to …", keep the version before it.
        seen_version: match &status.updated_to {
            Some(_) => load_saved(app).and_then(|s| s.seen_version),
            None => Some(current),
        },
    };
    if let Ok(text) = serde_json::to_string_pretty(&saved) {
        let _ = std::fs::write(path, text);
    }
}

/// Changes the status, keeps what survives a restart, and tells the interface.
fn set(app: &AppHandle, change: impl FnOnce(&mut Status)) {
    let state = app.state::<Updates>();
    let status = {
        let mut s = lock(&state.status);
        change(&mut s);
        s.clone()
    };
    save(app, &status);
    let _ = app.emit("update-status", &status);
}

/// Progress only: nothing to keep on disk.
fn set_progress(app: &AppHandle, done: u64, total: Option<u64>) {
    let state = app.state::<Updates>();
    let status = {
        let mut s = lock(&state.status);
        s.done = done;
        s.total = total;
        s.clone()
    };
    let _ = app.emit("update-status", &status);
}

pub fn watch(app: AppHandle) {
    let dev = cfg!(debug_assertions);
    let current = app.package_info().version.to_string();
    let saved = load_saved(&app);
    let updated_to = match &saved {
        // Dev builds run whatever the source says; nothing was installed.
        _ if dev => None,
        Some(Saved { seen_version: Some(seen), .. }) => (*seen != current).then(|| current.clone()),
        // No version on record: an older Hearth updated to this one (0.3.0
        // kept none), unless this is a fresh install that was never opened.
        _ => app
            .path()
            .app_data_dir()
            .ok()
            .filter(|d| d.join("prefs.json").exists())
            .map(|_| current.clone()),
    };
    set(&app, |s| {
        s.dev = dev;
        s.last_checked = saved.as_ref().and_then(|x| x.last_checked);
        s.error = saved.as_ref().and_then(|x| x.last_error.clone());
        s.updated_to = updated_to;
    });
    // Dev builds run from the source tree; the installer would replace the wrong thing.
    if dev {
        return;
    }
    tauri::async_runtime::spawn(async move {
        // Let the first sync have the network.
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            check(&app).await;
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// Checks GitHub and downloads a newer Hearth. Does nothing while a check or
/// download is already running, or while installing.
async fn check(app: &AppHandle) {
    let state = app.state::<Updates>();
    {
        let mut s = lock(&state.status);
        if matches!(s.phase, Phase::Checking | Phase::Downloading | Phase::Installing) {
            return;
        }
        s.phase = Phase::Checking;
    }
    set(app, |_| {});
    match fetch(app).await {
        Ok(()) => {}
        Err(e) => {
            tracking::log(app, &format!("update: couldn't check ({e})"));
            let waiting = lock(&state.ready).is_some();
            set(app, |s| {
                s.phase = if waiting { Phase::Ready } else { Phase::Idle };
                if !waiting {
                    s.version = None;
                    s.notes = None;
                }
                s.error = Some(reason(&e));
            });
        }
    }
}

async fn fetch(app: &AppHandle) -> tauri_plugin_updater::Result<()> {
    let state = app.state::<Updates>();
    let found = app.updater()?.check().await?;
    let Some(update) = found else {
        set(app, |s| {
            s.phase = Phase::Idle;
            s.version = None;
            s.notes = None;
            s.last_checked = Some(now());
            s.error = None;
        });
        return Ok(());
    };
    let version = update.version.clone();
    if lock(&state.ready).as_ref().is_some_and(|(u, _)| u.version == version) {
        set(app, |s| {
            s.phase = Phase::Ready;
            s.last_checked = Some(now());
            s.error = None;
        });
        return Ok(());
    }
    set(app, |s| {
        s.phase = Phase::Downloading;
        s.version = Some(version.clone());
        s.notes = update.body.clone().filter(|b| !b.trim().is_empty());
        s.done = 0;
        s.total = None;
        s.last_checked = Some(now());
        s.error = None;
    });
    let mut done = 0u64;
    let mut last = Instant::now();
    let bytes = update
        .download(
            |chunk, total| {
                done += chunk as u64;
                if last.elapsed() >= PROGRESS_EVERY {
                    last = Instant::now();
                    set_progress(app, done, total);
                }
            },
            || {},
        )
        .await?;
    *lock(&state.ready) = Some((update, bytes));
    tray::set_update(app, Some(&version));
    set(app, |s| s.phase = Phase::Ready);
    Ok(())
}

/// Why a check failed, in words for the Settings row.
fn reason(e: &UpdateError) -> String {
    match e {
        UpdateError::Reqwest(_) | UpdateError::Network(_) | UpdateError::Http(_) => {
            "Couldn't reach GitHub. Check your internet connection.".into()
        }
        UpdateError::ReleaseNotFound | UpdateError::Serialization(_) | UpdateError::TargetNotFound(_) | UpdateError::TargetsNotFound(_) => {
            "GitHub didn't send a usable answer. Hearth will try again later.".into()
        }
        UpdateError::Minisign(_)
        | UpdateError::Base64(_)
        | UpdateError::SignatureUtf8(_)
        | UpdateError::SignedVersionMismatch { .. }
        | UpdateError::MissingSignedVersion => "The update didn't pass Hearth's safety check, so it wasn't installed.".into(),
        UpdateError::Io(_) => "Couldn't save the download. Check there's space on your system drive.".into(),
        other => format!("Something went wrong: {other}"),
    }
}

/// Closes Hearth, runs the installer and starts the new version.
pub fn install(app: &AppHandle) -> CmdResult<()> {
    let state = app.state::<Updates>();
    let Some((update, bytes)) = lock(&state.ready).take() else {
        return Err("No update is ready.".into());
    };
    set(app, |s| {
        s.phase = Phase::Installing;
        s.error = None;
    });
    tracking::flush(app);
    if let Err(e) = update.install(&bytes) {
        let reason = format!("Couldn't install it: {e}");
        *lock(&state.ready) = Some((update, bytes));
        set(app, |s| {
            s.phase = Phase::Ready;
            s.error = Some(reason.clone());
        });
        return Err(reason);
    }
    app.restart();
}

/// The section of CHANGELOG.md for one version: everything under its
/// `## <version>` heading, up to the next version's.
fn changelog_section(changelog: &str, version: &str) -> Option<String> {
    let heading = format!("## {version}");
    let mut lines = changelog.lines().skip_while(|l| l.trim_end() != heading);
    lines.next()?;
    let body: Vec<&str> = lines.take_while(|l| !l.starts_with("## ")).collect();
    let text = body.join("\n").trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[tauri::command]
pub fn update_status(state: State<Updates>) -> Status {
    lock(&state.status).clone()
}

/// What changed in the running version, from the built-in changelog.
#[tauri::command]
pub fn release_notes(app: AppHandle) -> Option<String> {
    changelog_section(CHANGELOG, &app.package_info().version.to_string())
}

#[tauri::command]
pub async fn check_updates(app: AppHandle) -> CmdResult<()> {
    if cfg!(debug_assertions) {
        return Err("Development builds don't check for updates.".into());
    }
    check(&app).await;
    Ok(())
}

/// The user has seen "Updated to …": don't say it again.
#[tauri::command]
pub fn seen_update(app: AppHandle) {
    set(&app, |s| s.updated_to = None);
}

#[tauri::command]
pub fn install_update(app: AppHandle) -> CmdResult<()> {
    install(&app)
}

#[cfg(test)]
mod tests {
    use super::changelog_section;

    const LOG: &str = "# Changelog\n\nIntro.\n\n## Unreleased\n\n- Next\n\n## 0.3.0\n\nA big one.\n\n### New\n\n- **Storage.** Sizes.\n\n## 0.2.1\n\n- Fix\n";

    #[test]
    fn takes_one_versions_section() {
        assert_eq!(changelog_section(LOG, "0.3.0").as_deref(), Some("A big one.\n\n### New\n\n- **Storage.** Sizes."));
        assert_eq!(changelog_section(LOG, "0.2.1").as_deref(), Some("- Fix"));
    }

    #[test]
    fn missing_versions_have_none() {
        assert_eq!(changelog_section(LOG, "0.4.0"), None);
        // "## 0.3" must not match "## 0.3.0".
        assert_eq!(changelog_section(LOG, "0.3"), None);
    }
}
