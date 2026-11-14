//! The trade site session (POESESSID) for live searches and buying: kept in the
//! system's credential store (Windows Credential Manager), never in a file, and only
//! ever sent to pathofexile.com.

const SERVICE: &str = "poe-tool";
const USER: &str = "POESESSID";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, USER).map_err(|e| e.to_string())
}

pub fn get() -> Option<String> {
    entry().ok()?.get_password().ok().filter(|s| !s.is_empty())
}

pub fn set(value: &str) -> Result<(), String> {
    let value = value.trim().trim_start_matches("POESESSID=");
    if value.len() < 16 || !value.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("That doesn't look like a POESESSID (a long string of letters and digits).".into());
    }
    entry()?.set_password(value).map_err(|e| e.to_string())
}

pub fn clear() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
