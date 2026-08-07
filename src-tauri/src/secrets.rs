//! OS keychain integration for secrets (API keys).
//!
//! All operations are best-effort: when the platform keychain is unavailable
//! (e.g. sandboxed tests, headless environments, no keychain daemon), callers
//! fall back to the legacy settings.json storage.

const SERVICE: &str = "com.deepseek.app";

/// Read a secret from the OS keychain.
/// Returns `Ok(None)` when the entry does not exist or the keychain is unavailable.
pub fn get(key: &str) -> Option<String> {
    let entry = keyring::Entry::new(SERVICE, key).ok()?;
    match entry.get_password() {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

/// Write a secret to the OS keychain.
/// Returns `false` if the keychain is unavailable (caller should fall back).
pub fn set(key: &str, value: &str) -> bool {
    let entry = match keyring::Entry::new(SERVICE, key) {
        Ok(e) => e,
        Err(_) => return false,
    };
    match entry.set_password(value) {
        Ok(_) => true,
        Err(_) => false,
    }
}

/// Remove a secret from the OS keychain. Best-effort.
pub fn delete(key: &str) {
    if let Ok(entry) = keyring::Entry::new(SERVICE, key) {
        let _ = entry.delete_credential();
    }
}
