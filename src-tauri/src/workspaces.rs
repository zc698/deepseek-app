//! Workspace registry: a set of trusted folders that the agent operates in.
//!
//! A workspace is a user-trusted directory used as the tool sandbox root (the
//! agent's cwd), mirroring grok-app's "project = trusted folder" model. State
//! is persisted to `<data_dir>/workspaces.json`; on first launch the legacy
//! `settings.workspace_dir` (or `$HOME`) is seeded as the initial workspace.

use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub current: Option<String>,
    pub items: Vec<Workspace>,
}

impl WorkspaceState {
    pub fn current_workspace(&self) -> Option<&Workspace> {
        self.items.iter().find(|w| Some(&w.id) == self.current.as_ref())
    }

    pub fn current_path(&self) -> Option<&str> {
        self.current_workspace().map(|w| w.path.as_str())
    }
}

pub struct WorkspaceStore {
    path: PathBuf,
}

impl WorkspaceStore {
    pub fn new(root: &Path) -> Self {
        WorkspaceStore {
            path: root.join("workspaces.json"),
        }
    }

    pub fn load(&self) -> WorkspaceState {
        match std::fs::read_to_string(&self.path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => WorkspaceState::default(),
        }
    }

    pub fn save(&self, state: &WorkspaceState) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(crate::error::AppError::io)?;
        }
        let raw = serde_json::to_string_pretty(state)?;
        std::fs::write(&self.path, raw).map_err(crate::error::AppError::io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

/// Seed the registry on first launch: the legacy `settings.workspace_dir`
/// (falling back to `$HOME`) becomes the first workspace, matching grok-app's
/// "no project -> $HOME" orphan semantics. Never overwrites existing state.
pub fn seed_from_settings(data_dir: &Path, legacy_dir: &str) -> WorkspaceState {
    let store = WorkspaceStore::new(data_dir);
    let mut state = store.load();
    if !state.items.is_empty() {
        return state;
    }
    let path = if legacy_dir.trim().is_empty() {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .to_string_lossy()
            .into_owned()
    } else {
        legacy_dir.to_string()
    };
    let name = Path::new(&path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "默认工作区".to_string());
    let w = Workspace {
        id: Uuid::new_v4().to_string(),
        name,
        path,
    };
    state.current = Some(w.id.clone());
    state.items.push(w);
    let _ = store.save(&state);
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> String {
        dirs::home_dir().unwrap().to_string_lossy().into_owned()
    }

    #[test]
    fn seeds_first_workspace_from_legacy_dir() {
        let dir = tempfile::tempdir().unwrap();
        let state = seed_from_settings(dir.path(), "/tmp/legacy");
        assert_eq!(state.items.len(), 1);
        assert_eq!(state.items[0].path, "/tmp/legacy");
        assert_eq!(state.items[0].name, "legacy");
        assert_eq!(state.current.as_deref(), Some(state.items[0].id.as_str()));

        // Second launch must not duplicate.
        let state2 = seed_from_settings(dir.path(), "/tmp/legacy");
        assert_eq!(state2.items.len(), 1);
    }

    #[test]
    fn seeds_home_when_no_legacy_dir() {
        let dir = tempfile::tempdir().unwrap();
        let state = seed_from_settings(dir.path(), "");
        assert_eq!(state.items.len(), 1);
        assert_eq!(state.items[0].path, home());
    }

    #[test]
    fn roundtrip_preserves_state() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorkspaceStore::new(dir.path());
        let mut state = seed_from_settings(dir.path(), &home());
        state.items.push(Workspace {
            id: "w2".into(),
            name: "项目B".into(),
            path: "/tmp/b".into(),
        });
        store.save(&state).unwrap();
        let loaded = WorkspaceStore::new(dir.path()).load();
        assert_eq!(loaded.items.len(), 2);
        assert_eq!(loaded.current, state.current);
        assert_eq!(loaded.items[1].name, "项目B");
    }

    #[test]
    fn current_workspace_lookup() {
        let state = WorkspaceState {
            current: Some("w2".into()),
            items: vec![
                Workspace { id: "w1".into(), name: "a".into(), path: "/a".into() },
                Workspace { id: "w2".into(), name: "b".into(), path: "/b".into() },
            ],
        };
        assert_eq!(state.current_workspace().map(|w| w.name.as_str()), Some("b"));
        assert_eq!(state.current_path(), Some("/b"));
        let none = WorkspaceState::default();
        assert_eq!(none.current_path(), None);
    }
}
