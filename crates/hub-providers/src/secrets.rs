//! Sign-in tokens live in Windows Credential Manager, never in plain files.
//!
//! Credential Manager holds at most 2,560 bytes per secret, and keyring stores
//! text as UTF-16, so about 1,280 characters fit. An Epic sign-in is longer
//! than that, so long secrets are split across numbered entries
//! (`epic.1`, `epic.2`, ...) with `epic` itself recording how many there are.

const SERVICE: &str = "dev.hearth.app";
/// Characters per entry, safely under the 1,280 limit.
const CHUNK: usize = 1200;
const CHUNKED: &str = "chunked:";

#[cfg(windows)]
fn entry(service: &str, account: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(service, account).map_err(|e| e.to_string())
}

#[cfg(windows)]
pub fn load(account: &str) -> Option<String> {
    load_in(SERVICE, account)
}

/// Stores `secret` (or deletes it with None). Errors say why the secret
/// couldn't be saved, so callers can report it instead of losing a sign-in.
#[cfg(windows)]
pub fn save(account: &str, secret: Option<&str>) -> Result<(), String> {
    save_in(SERVICE, account, secret)
}

#[cfg(windows)]
fn load_in(service: &str, account: &str) -> Option<String> {
    let head = entry(service, account).ok()?.get_password().ok()?;
    let Some(n) = head.strip_prefix(CHUNKED) else { return Some(head) };
    let n: usize = n.parse().ok()?;
    let mut out = String::new();
    for i in 1..=n {
        out.push_str(&entry(service, &format!("{account}.{i}")).ok()?.get_password().ok()?);
    }
    Some(out)
}

#[cfg(windows)]
fn save_in(service: &str, account: &str, secret: Option<&str>) -> Result<(), String> {
    // Parts the previous value used, to clear any the new one doesn't need.
    let old_parts = entry(service, account)
        .ok()
        .and_then(|e| e.get_password().ok())
        .and_then(|h| h.strip_prefix(CHUNKED).and_then(|n| n.parse::<usize>().ok()))
        .unwrap_or(0);
    let head = entry(service, account)?;

    let parts: Vec<String> = match secret {
        None => {
            let _ = head.delete_credential();
            Vec::new()
        }
        Some(s) if s.chars().count() <= CHUNK && !s.starts_with(CHUNKED) => {
            head.set_password(s).map_err(|e| e.to_string())?;
            Vec::new()
        }
        Some(s) => {
            let chars: Vec<char> = s.chars().collect();
            let parts: Vec<String> = chars.chunks(CHUNK).map(|c| c.iter().collect()).collect();
            for (i, part) in parts.iter().enumerate() {
                entry(service, &format!("{account}.{}", i + 1))?
                    .set_password(part)
                    .map_err(|e| e.to_string())?;
            }
            // Written last, so a half-finished save never points at missing parts.
            head.set_password(&format!("{CHUNKED}{}", parts.len())).map_err(|e| e.to_string())?;
            parts
        }
    };
    for i in parts.len() + 1..=old_parts {
        if let Ok(e) = entry(service, &format!("{account}.{i}")) {
            let _ = e.delete_credential();
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn load(_account: &str) -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn save(_account: &str, _secret: Option<&str>) -> Result<(), String> {
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    // Uses a separate service name, and cleans up after itself.
    const TEST: &str = "dev.hearth.app.test";

    // One test, run in order: Credential Manager left entries behind when
    // two tests wrote to it from parallel threads.
    #[test]
    fn long_secrets_round_trip_and_clean_up() {
        let long: String = (0..3000).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
        save_in(TEST, "long", Some(&long)).unwrap();
        assert_eq!(load_in(TEST, "long").as_deref(), Some(long.as_str()));

        // Shrinking to a short value removes the old parts.
        save_in(TEST, "long", Some("short")).unwrap();
        assert_eq!(load_in(TEST, "long").as_deref(), Some("short"));
        assert!(entry(TEST, "long.1").unwrap().get_password().is_err());

        save_in(TEST, "long", None).unwrap();
        assert_eq!(load_in(TEST, "long"), None);

        // Deleting a long secret removes every part.
        save_in(TEST, "gone", Some(&"x".repeat(2500))).unwrap();
        save_in(TEST, "gone", None).unwrap();
        assert_eq!(load_in(TEST, "gone"), None);
        for i in 1..=3 {
            assert!(entry(TEST, &format!("gone.{i}")).unwrap().get_password().is_err());
        }
    }
}

