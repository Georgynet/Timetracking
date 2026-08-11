use keyring::Entry;

/// Must match `tauri.conf.json`'s `identifier` — keeps the keychain entry namespaced
/// to this app specifically.
const SERVICE_NAME: &str = "com.georg.timetracking";
const KEYCHAIN_USERNAME: &str = "jira_api_token";

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error(
        "Could not access the system keychain ({0}). On Linux, make sure a Secret \
         Service provider (e.g. gnome-keyring or KWallet) is running."
    )]
    Backend(String),
}

fn entry() -> Result<Entry, SecretError> {
    Entry::new(SERVICE_NAME, KEYCHAIN_USERNAME).map_err(|e| SecretError::Backend(e.to_string()))
}

pub fn store_token(token: &str) -> Result<(), SecretError> {
    entry()?
        .set_password(token)
        .map_err(|e| SecretError::Backend(e.to_string()))
}

pub fn load_token() -> Result<Option<String>, SecretError> {
    match entry()?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(SecretError::Backend(e.to_string())),
    }
}

pub fn delete_token() -> Result<(), SecretError> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(SecretError::Backend(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Touches the *real* OS keychain (macOS Keychain / Linux Secret Service) — not
    /// run by default `cargo test` since it needs an interactive/unlocked keychain
    /// session. Run explicitly with `cargo test -- --ignored keyring_round_trip`.
    #[test]
    #[ignore]
    fn real_keychain_round_trip() {
        let probe_token = "smoke-test-token-12345";
        store_token(probe_token).expect("should be able to write to the real keychain");
        let loaded = load_token().expect("should be able to read back from the real keychain");
        assert_eq!(loaded.as_deref(), Some(probe_token));
        delete_token().expect("should be able to delete from the real keychain");
        assert_eq!(load_token().unwrap(), None);
    }
}
