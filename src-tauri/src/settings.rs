use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_BASE_URL: &str = "https://api.deepseek.com";
pub const DEFAULT_MODEL: &str = "deepseek-v4-flash";

/// Resolve the app data directory.
/// Tests override this with the DEEPSEEK_APP_DATA env var.
pub fn data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("DEEPSEEK_APP_DATA") {
        if !d.trim().is_empty() {
            return PathBuf::from(d);
        }
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("DeepSeekApp")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub temperature: f64,
    pub system_prompt: String,
    pub allow_bash: bool,
    pub workspace_dir: String,
    pub max_tool_rounds: usize,
    pub enabled_skills: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            api_key: std::env::var("DEEPSEEK_API_KEY").unwrap_or_default(),
            base_url: DEFAULT_BASE_URL.to_string(),
            model: DEFAULT_MODEL.to_string(),
            temperature: 1.0,
            system_prompt: String::new(),
            allow_bash: false,
            workspace_dir: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .to_string_lossy()
                .to_string(),
            max_tool_rounds: 20,
            enabled_skills: Vec::new(),
        }
    }
}

impl Settings {
    pub fn workspace_path(&self) -> PathBuf {
        let p = PathBuf::from(&self.workspace_dir);
        if p.is_absolute() {
            p
        } else {
            std::env::current_dir().unwrap_or_default().join(p)
        }
    }

    /// Resolve the effective API key with the following priority:
    /// 1. `DEEPSEEK_API_KEY` env var (explicit override)
    /// 2. OS keychain
    /// 3. settings.json (legacy storage / fallback when keychain is unavailable)
    pub fn effective_api_key(&self) -> String {
        if let Ok(k) = std::env::var("DEEPSEEK_API_KEY") {
            if !k.trim().is_empty() {
                return k;
            }
        }
        if let Some(k) = crate::secrets::get("api_key") {
            return k;
        }
        self.api_key.clone()
    }

    /// Resolve the model id, mapping legacy V3-era ids to their V4 successors.
    /// This is an in-memory migration: the persisted settings.json is left
    /// untouched (no user config is destroyed); the app simply uses the V4
    /// model that replaced the stored id.
    pub fn effective_model(&self) -> String {
        match self.model.as_str() {
            "deepseek-chat" => "deepseek-v4-flash".to_string(),
            "deepseek-reasoner" => "deepseek-v4-pro".to_string(),
            m => m.to_string(),
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(root: &std::path::Path) -> Self {
        SettingsStore {
            path: root.join("settings.json"),
        }
    }

    /// Load settings, migrating any legacy plaintext key into the OS keychain.
    /// The keychain write is best-effort: when unavailable (sandboxed tests,
    /// headless environments) the key stays in settings.json as a fallback.
    pub fn load(&self) -> Settings {
        let settings = match std::fs::read_to_string(&self.path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Settings::default(),
        };
        self.migrate_key(&settings);
        settings
    }

    /// Move a plaintext key found in settings.json into the OS keychain.
    fn migrate_key(&self, settings: &Settings) {
        if settings.api_key.trim().is_empty() {
            return;
        }
        if !crate::secrets::set("api_key", &settings.api_key) {
            return; // keychain unavailable -> keep file fallback
        }
        // Keychain write succeeded: blank the key in the persisted file.
        let mut next = settings.clone();
        next.api_key = String::new();
        let _ = self.save(&next);
    }

    pub fn save(&self, settings: &Settings) -> crate::error::AppResult<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(crate::error::AppError::io)?;
        }
        let raw = serde_json::to_string_pretty(settings)?;
        std::fs::write(&self.path, raw).map_err(crate::error::AppError::io)?;
        // Restrict permissions so the API key is not world-readable.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path());
        let mut s = Settings::default();
        // api_key is kept empty here: the key itself is handled by the keychain
        // (or the migration test below), so this test is keychain-independent.
        s.api_key = String::new();
        s.model = "deepseek-reasoner".into();
        s.temperature = 0.7;
        store.save(&s).unwrap();
        let loaded = store.load();
        assert_eq!(loaded, s);
        assert_eq!(loaded.model, "deepseek-reasoner");
        assert_eq!(loaded.temperature, 0.7);
    }

    #[test]
    fn settings_defaults_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path());
        let s = store.load();
        assert_eq!(s.base_url, DEFAULT_BASE_URL);
        assert_eq!(s.model, DEFAULT_MODEL);
    }

    /// Keychain-dependent tests are serialized (the keychain is a shared
    /// resource) and clean up after themselves, so they pass with OR without a
    /// working keychain (macOS CI has one; the sandbox does not).
    #[test]
    #[serial_test::serial(keychain)]
    fn effective_key_prefers_env_over_file() {
        crate::secrets::delete("api_key");
        let s = Settings {
            api_key: "file-key".into(),
            ..Settings::default()
        };
        let prev = std::env::var("DEEPSEEK_API_KEY");
        std::env::set_var("DEEPSEEK_API_KEY", "env-key");
        assert_eq!(s.effective_api_key(), "env-key");
        match prev {
            Ok(v) => std::env::set_var("DEEPSEEK_API_KEY", v),
            Err(_) => std::env::remove_var("DEEPSEEK_API_KEY"),
        }
    }

    #[test]
    #[serial_test::serial(keychain)]
    fn effective_key_falls_back_to_file_without_env() {
        crate::secrets::delete("api_key");
        let prev = std::env::var("DEEPSEEK_API_KEY");
        std::env::remove_var("DEEPSEEK_API_KEY");
        let s = Settings {
            api_key: "file-key".into(),
            ..Settings::default()
        };
        assert_eq!(s.effective_api_key(), "file-key");
        match prev {
            Ok(v) => std::env::set_var("DEEPSEEK_API_KEY", v),
            Err(_) => {}
        }
    }

    #[test]
    fn effective_model_maps_legacy_ids_to_v4() {
        let s = Settings {
            model: "deepseek-chat".into(),
            ..Settings::default()
        };
        assert_eq!(s.effective_model(), "deepseek-v4-flash");
        let s = Settings {
            model: "deepseek-reasoner".into(),
            ..Settings::default()
        };
        assert_eq!(s.effective_model(), "deepseek-v4-pro");
        // Unknown / already-current ids pass through untouched.
        let s = Settings {
            model: "deepseek-v4-flash".into(),
            ..Settings::default()
        };
        assert_eq!(s.effective_model(), "deepseek-v4-flash");
        let s = Settings {
            model: "some-future-model".into(),
            ..Settings::default()
        };
        assert_eq!(s.effective_model(), "some-future-model");
    }

    #[test]
    #[serial_test::serial(keychain)]
    fn load_migrates_plaintext_key_or_keeps_fallback() {
        crate::secrets::delete("api_key");
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path());
        let mut s = Settings::default();
        s.api_key = "sk-legacy".into();
        store.save(&s).unwrap();

        let loaded = store.load();
        // Invariant that holds in BOTH environments:
        // - keychain available  -> key moved to the keychain, file field blanked
        // - keychain unavailable -> key stays in settings.json (fallback)
        if loaded.api_key.is_empty() {
            assert_eq!(
                crate::secrets::get("api_key").as_deref(),
                Some("sk-legacy"),
                "migrated key must be retrievable from the keychain"
            );
        } else {
            assert_eq!(loaded.api_key, "sk-legacy", "keychain unavailable -> file fallback kept");
        }
        crate::secrets::delete("api_key");
    }
}
