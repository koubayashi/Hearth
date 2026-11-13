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

/// What a chunked head points at: how many parts, and the name prefix they
/// share. Parts alternate between two sets (`epic.a.1…`, `epic.b.1…`) so a
/// save never overwrites the set the head still points at; heads written
/// before that (`chunked:N`, parts `epic.1…`) still load.
#[cfg(windows)]
fn parts_of(account: &str, head: &str) -> Option<(usize, String)> {
    let rest = head.strip_prefix(CHUNKED)?;
    match rest.split_once(':') {
        Some((n, set)) => Some((n.parse().ok()?, format!("{account}.{set}"))),
        None => Some((rest.parse().ok()?, account.to_string())),
    }
}

#[cfg(windows)]
fn load_in(service: &str, account: &str) -> Option<String> {
    let head = entry(service, account).ok()?.get_password().ok()?;
    if !head.starts_with(CHUNKED) {
        return Some(head);
    }
    let (n, prefix) = parts_of(account, &head)?;
    let mut out = String::new();
    for i in 1..=n {
        out.push_str(&entry(service, &format!("{prefix}.{i}")).ok()?.get_password().ok()?);
    }
    Some(out)
}

#[cfg(windows)]
fn save_in(service: &str, account: &str, secret: Option<&str>) -> Result<(), String> {
    // The parts the previous value used, cleared once the head moves on.
    let old = entry(service, account).ok().and_then(|e| e.get_password().ok()).and_then(|h| parts_of(account, &h));
    let head = entry(service, account)?;

    match secret {
        None => {
            let _ = head.delete_credential();
        }
        Some(s) if s.chars().count() <= CHUNK && !s.starts_with(CHUNKED) => {
            head.set_password(s).map_err(|e| e.to_string())?;
        }
        Some(s) => {
            // Write into the set the head doesn't point at, so a crash
            // midway leaves the old sign-in whole.
            let set = if old.as_ref().is_some_and(|(_, p)| *p == format!("{account}.a")) { "b" } else { "a" };
            let prefix = format!("{account}.{set}");
            let chars: Vec<char> = s.chars().collect();
            let parts: Vec<String> = chars.chunks(CHUNK).map(|c| c.iter().collect()).collect();
            for (i, part) in parts.iter().enumerate() {
                entry(service, &format!("{prefix}.{}", i + 1))?
                    .set_password(part)
                    .map_err(|e| e.to_string())?;
            }
            // Written last, so a half-finished save never points at missing parts.
            head.set_password(&format!("{CHUNKED}{}:{set}", parts.len())).map_err(|e| e.to_string())?;
            // Leftovers in this set from a save that crashed before cleaning up.
            let mut i = parts.len() + 1;
            while entry(service, &format!("{prefix}.{i}")).is_ok_and(|e| e.delete_credential().is_ok()) {
                i += 1;
            }
        }
    }
    if let Some((n, prefix)) = old {
        for i in 1..=n {
            if let Ok(e) = entry(service, &format!("{prefix}.{i}")) {
                let _ = e.delete_credential();
            }
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
        assert!(entry(TEST, "long.a.1").unwrap().get_password().is_err());

        save_in(TEST, "long", None).unwrap();
        assert_eq!(load_in(TEST, "long"), None);

        // Deleting a long secret removes every part.
        save_in(TEST, "gone", Some(&"x".repeat(2500))).unwrap();
        save_in(TEST, "gone", None).unwrap();
        assert_eq!(load_in(TEST, "gone"), None);
        for set in ["a", "b"] {
            for i in 1..=3 {
                assert!(entry(TEST, &format!("gone.{set}.{i}")).unwrap().get_password().is_err());
            }
        }

        // Replacing a long secret writes the other set and clears the old one,
        // so a crash midway would have left the old sign-in readable.
        save_in(TEST, "swap", Some(&"1".repeat(2500))).unwrap();
        save_in(TEST, "swap", Some(&"2".repeat(1300))).unwrap();
        assert_eq!(load_in(TEST, "swap"), Some("2".repeat(1300)));
        assert!(entry(TEST, "swap.a.1").unwrap().get_password().is_err());
        save_in(TEST, "swap", Some(&"3".repeat(2500))).unwrap();
        assert_eq!(load_in(TEST, "swap"), Some("3".repeat(2500)));
        assert!(entry(TEST, "swap.b.1").unwrap().get_password().is_err());
        save_in(TEST, "swap", None).unwrap();

        // A head saved in the old format still loads, and its parts are
        // cleared on the next save.
        entry(TEST, "old.1").unwrap().set_password(&"o".repeat(1200)).unwrap();
        entry(TEST, "old.2").unwrap().set_password("p").unwrap();
        entry(TEST, "old").unwrap().set_password("chunked:2").unwrap();
        assert_eq!(load_in(TEST, "old"), Some(format!("{}p", "o".repeat(1200))));
        save_in(TEST, "old", Some(&"n".repeat(1300))).unwrap();
        assert_eq!(load_in(TEST, "old"), Some("n".repeat(1300)));
        assert!(entry(TEST, "old.1").unwrap().get_password().is_err());
        save_in(TEST, "old", None).unwrap();
        assert!(entry(TEST, "old.a.1").unwrap().get_password().is_err());
    }
}

