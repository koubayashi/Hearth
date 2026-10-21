//! Sign-in tokens live in Windows Credential Manager, never in plain files.

const SERVICE: &str = "dev.hearth.app";

#[cfg(windows)]
pub fn load(account: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, account).ok()?.get_password().ok()
}

#[cfg(windows)]
pub fn save(account: &str, secret: Option<&str>) {
    let Ok(entry) = keyring::Entry::new(SERVICE, account) else { return };
    let _ = match secret {
        Some(s) => entry.set_password(s),
        None => entry.delete_credential(),
    };
}

#[cfg(not(windows))]
pub fn load(_account: &str) -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn save(_account: &str, _secret: Option<&str>) {}
