use crate::agent::{AgentOutput, StoredTool};
use crate::error::{AppError, AppResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub reasoning: String,
    #[serde(default)]
    pub tools: Vec<StoredTool>,
    #[serde(default)]
    pub is_error: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFile {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub messages: Vec<StoredMessage>,
    /// Raw OpenAI-format conversation, used to continue a session faithfully.
    #[serde(default)]
    pub api_messages: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct SessionStore {
    dir: PathBuf,
}

impl SessionStore {
    pub fn new(root: &Path) -> Self {
        SessionStore {
            dir: root.join("sessions"),
        }
    }

    fn ensure(&self) -> AppResult<()> {
        std::fs::create_dir_all(&self.dir).map_err(AppError::io)
    }

    fn path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    pub fn list(&self) -> AppResult<Vec<SessionMeta>> {
        self.ensure()?;
        let mut metas = Vec::new();
        for entry in std::fs::read_dir(&self.dir).map_err(AppError::io)? {
            let entry = entry.map_err(AppError::io)?;
            if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(raw) = std::fs::read_to_string(entry.path()) {
                if let Ok(sf) = serde_json::from_str::<SessionFile>(&raw) {
                    metas.push(SessionMeta {
                        id: sf.id,
                        title: sf.title,
                        created_at: sf.created_at,
                        updated_at: sf.updated_at,
                    });
                }
            }
        }
        metas.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(metas)
    }

    pub fn create(&self) -> AppResult<SessionFile> {
        self.ensure()?;
        let now = Utc::now().to_rfc3339();
        let sf = SessionFile {
            id: Uuid::new_v4().to_string(),
            title: String::new(),
            created_at: now.clone(),
            updated_at: now,
            messages: Vec::new(),
            api_messages: Vec::new(),
        };
        self.save(&sf)?;
        Ok(sf)
    }

    pub fn load(&self, id: &str) -> AppResult<SessionFile> {
        let raw = std::fs::read_to_string(self.path(id)).map_err(AppError::io)?;
        serde_json::from_str(&raw).map_err(AppError::from)
    }

    pub fn delete(&self, id: &str) -> AppResult<()> {
        match std::fs::remove_file(self.path(id)) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(AppError::io(e)),
        }
    }

    fn save(&self, sf: &SessionFile) -> AppResult<()> {
        let raw = serde_json::to_string_pretty(sf)?;
        std::fs::write(self.path(&sf.id), raw).map_err(AppError::io)
    }

    /// Record the user message; derive the session title on first message.
    pub fn append_user(&self, id: &str, text: &str) -> AppResult<()> {
        let mut sf = self.load(id)?;
        sf.messages.push(StoredMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".into(),
            content: text.to_string(),
            reasoning: String::new(),
            tools: Vec::new(),
            is_error: false,
            created_at: Utc::now().to_rfc3339(),
        });
        if sf.title.is_empty() {
            let t = text.trim().chars().take(30).collect::<String>();
            sf.title = if text.trim().chars().count() > 30 {
                format!("{t}…")
            } else {
                t
            };
        }
        sf.updated_at = Utc::now().to_rfc3339();
        self.save(&sf)
    }

    /// Record the assistant turn (content + reasoning + tools + raw api messages).
    pub fn append_assistant(
        &self,
        id: &str,
        output: &AgentOutput,
        is_error: bool,
    ) -> AppResult<()> {
        let mut sf = self.load(id)?;
        sf.messages.push(StoredMessage {
            id: Uuid::new_v4().to_string(),
            role: "assistant".into(),
            content: output.content.clone(),
            reasoning: output.reasoning.clone(),
            tools: output.tools.clone(),
            is_error,
            created_at: Utc::now().to_rfc3339(),
        });
        sf.api_messages = output.api_messages.clone();
        sf.updated_at = Utc::now().to_rfc3339();
        self.save(&sf)
    }
}
