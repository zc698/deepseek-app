use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_BASE_URL: &str = "https://api.deepseek.com";
pub const DEFAULT_MODEL: &str = "deepseek-chat";

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

    pub fn load(&self) -> Settings {
        match std::fs::read_to_string(&self.path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
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
        s.api_key = "sk-test-123".into();
        s.model = "deepseek-reasoner".into();
        s.temperature = 0.7;
        store.save(&s).unwrap();
        let loaded = store.load();
        assert_eq!(loaded, s);
        assert_eq!(loaded.api_key, "sk-test-123");
    }

    #[test]
    fn settings_defaults_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path());
        let s = store.load();
        assert_eq!(s.base_url, DEFAULT_BASE_URL);
        assert_eq!(s.model, DEFAULT_MODEL);
    }
}
