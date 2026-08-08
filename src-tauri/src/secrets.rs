//! OS keychain integration for secrets (API keys).
//!
//! All operations are best-effort: when the platform keychain is unavailable
//! (e.g. headless environments, no keychain daemon), callers fall back to the
//! legacy settings.json storage.
//!
//! Unit tests use an in-memory store via `cfg(test)` so they are deterministic
//! and never touch the real OS keychain (whose availability varies between the
//! dev sandbox and CI runners).

const SERVICE: &str = "com.deepseek.app";

#[cfg(test)]
fn test_store() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static STORE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Read a secret from the OS keychain.
/// Returns `None` when the entry does not exist or the keychain is unavailable.
pub fn get(key: &str) -> Option<String> {
    #[cfg(test)]
    {
        return test_store().lock().unwrap().get(key).cloned();
    }
    #[cfg(not(test))]
    {
        let entry = keyring::Entry::new(SERVICE, key).ok()?;
        match entry.get_password() {
            Ok(v) if !v.is_empty() => Some(v),
            _ => None,
        }
    }
}

/// Write a secret to the OS keychain.
/// Returns `false` if the keychain is unavailable (caller should fall back).
pub fn set(key: &str, value: &str) -> bool {
    #[cfg(test)]
    {
        test_store().lock().unwrap().insert(key.to_string(), value.to_string());
        return true;
    }
    #[cfg(not(test))]
    {
        let entry = match keyring::Entry::new(SERVICE, key) {
            Ok(e) => e,
            Err(_) => return false,
        };
        match entry.set_password(value) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

/// Remove a secret from the OS keychain. Best-effort.
pub fn delete(key: &str) {
    #[cfg(test)]
    {
        test_store().lock().unwrap().remove(key);
        return;
    }
    #[cfg(not(test))]
    {
        if let Ok(entry) = keyring::Entry::new(SERVICE, key) {
            let _ = entry.delete_credential();
        }
    }
}
