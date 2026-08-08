use crate::agent::{AgentConfig, AgentEvent, StoredTool};
use crate::deepseek::DeepSeekClient;
use crate::error::{AppError, AppResult};
use crate::sessions::{SessionMeta, SessionStore, StoredMessage};
use crate::settings::{Settings, SettingsStore};
use crate::skills::{self, SkillInfo};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

pub const EVENT_CHANNEL: &str = "chat://event";

pub struct AppState {
    pub data_dir: PathBuf,
    pub settings: RwLock<Settings>,
    pub sessions: Mutex<SessionStore>,
    pub workspaces: RwLock<crate::workspaces::WorkspaceState>,
    pub stop_flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

/// Payloads streamed to the frontend on EVENT_CHANNEL.
/// The `kind` tag matches the ChatEvent union in src/lib/types.ts.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ChatEventPayload {
    Start {
        session_id: String,
        message_id: String,
    },
    Stream {
        session_id: String,
        message_id: String,
        delta: String,
        reasoning: bool,
    },
    Tool {
        session_id: String,
        message_id: String,
        tool: StoredTool,
    },
    Done {
        session_id: String,
        message_id: String,
        content: String,
    },
    Error {
        session_id: String,
        message_id: String,
        error: String,
    },
}

fn skill_roots(data_dir: &std::path::Path) -> Vec<PathBuf> {
    vec![data_dir.join("skills")]
}

// ---- settings ----

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Settings {
    // Resolve the key from keychain (or env) and map legacy model ids to their
    // V4 successors so the settings UI shows the current model catalog.
    let settings = state.settings.read().unwrap().clone();
    let mut resolved = settings.clone();
    resolved.api_key = settings.effective_api_key();
    resolved.model = settings.effective_model();
    resolved
}

#[tauri::command]
pub fn settings_set(state: State<'_, AppState>, settings: Settings) -> AppResult<Settings> {
    if settings.base_url.trim().is_empty() {
        return Err(AppError::Config("base_url 不能为空".into()));
    }
    if settings.model.trim().is_empty() {
        return Err(AppError::Config("model 不能为空".into()));
    }
    if !(0.0..=2.0).contains(&settings.temperature) {
        return Err(AppError::Config("temperature 必须在 0~2 之间".into()));
    }
    // Persist the API key to the OS keychain; keep the file as a fallback only
    // when the keychain is unavailable. An empty key clears the stored secret.
    let mut to_save = settings.clone();
    if settings.api_key.trim().is_empty() {
        crate::secrets::delete("api_key");
    } else if crate::secrets::set("api_key", settings.api_key.trim()) {
        to_save.api_key = String::new();
    }
    // Persist the resolved (V4) model id so the file upgrades in place without
    // touching any other user setting.
    to_save.model = settings.effective_model();
    let store = SettingsStore::new(&state.data_dir);
    store.save(&to_save)?;
    *state.settings.write().unwrap() = settings.clone();
    Ok(settings)
}

// ---- sessions ----

#[tauri::command]
pub fn sessions_list(state: State<'_, AppState>) -> AppResult<Vec<SessionMeta>> {
    state.sessions.lock().unwrap().list()
}

#[tauri::command]
pub fn session_create(state: State<'_, AppState>) -> AppResult<String> {
    let sf = state.sessions.lock().unwrap().create()?;
    Ok(sf.id)
}

#[tauri::command]
pub fn session_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.sessions.lock().unwrap().delete(&id)
}

#[tauri::command]
pub fn session_messages(state: State<'_, AppState>, id: String) -> AppResult<Vec<StoredMessage>> {
    let sf = state.sessions.lock().unwrap().load(&id)?;
    Ok(sf.messages)
}

// ---- chat ----

#[tauri::command]
pub async fn chat_send(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: Option<String>,
    text: String,
) -> AppResult<Value> {
    if text.trim().is_empty() {
        return Err(AppError::Config("消息不能为空".into()));
    }
    let message_id = Uuid::new_v4().to_string();

    // Resolve (or create) the session and record the user message.
    let sid = match &session_id {
        Some(id) => id.clone(),
        None => state.sessions.lock().unwrap().create()?.id,
    };
    state.sessions.lock().unwrap().append_user(&sid, &text)?;
    let history = state.sessions.lock().unwrap().load(&sid)?.api_messages;

    // Emit a Start event so the frontend can create the assistant placeholder.
    let _ = app.emit(
        EVENT_CHANNEL,
        ChatEventPayload::Start {
            session_id: sid.clone(),
            message_id: message_id.clone(),
        },
    );

    // Snapshot owned data for the background task.
    let data_dir = state.data_dir.clone();
    let settings = state.settings.read().unwrap().clone();
    // The tool sandbox root follows the active workspace (grok-app style:
    // trusted folder = agent cwd), falling back to the legacy setting.
    let workspace_path = state
        .workspaces
        .read()
        .unwrap()
        .current_path()
        .map(PathBuf::from)
        .unwrap_or_else(|| settings.workspace_path());
    let stop_flags = state.stop_flags.clone();

    let stop = Arc::new(AtomicBool::new(false));
    stop_flags.lock().unwrap().insert(sid.clone(), stop.clone());

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();

    let app_fwd = app.clone();
    let fwd_sid = sid.clone();
    let fwd_mid = message_id.clone();
    let task_sid = sid.clone();
    let task_mid = message_id.clone();

    tauri::async_runtime::spawn(async move {
        // Forward agent events to the webview.
        let mut rx = event_rx;
        let fwd_app = app_fwd.clone();
        let forwarder_sid = task_sid.clone();
        let forwarder_mid = task_mid.clone();
        let forwarder = tauri::async_runtime::spawn(async move {
            while let Some(ev) = rx.recv().await {
                let payload = match ev {
                    AgentEvent::Stream { delta, reasoning } => ChatEventPayload::Stream {
                        session_id: forwarder_sid.clone(),
                        message_id: forwarder_mid.clone(),
                        delta,
                        reasoning,
                    },
                    AgentEvent::Tool { id, name, args, status, output } => {
                        ChatEventPayload::Tool {
                            session_id: forwarder_sid.clone(),
                            message_id: forwarder_mid.clone(),
                            tool: StoredTool {
                                id,
                                name,
                                args,
                                status: status.to_string(),
                                output,
                            },
                        }
                    }
                    AgentEvent::Done { content } => ChatEventPayload::Done {
                        session_id: forwarder_sid.clone(),
                        message_id: forwarder_mid.clone(),
                        content,
                    },
                    AgentEvent::Error { message } => ChatEventPayload::Error {
                        session_id: forwarder_sid.clone(),
                        message_id: forwarder_mid.clone(),
                        error: message,
                    },
                };
                let _ = fwd_app.emit(EVENT_CHANNEL, payload);
            }
        });

        // Run the agent.
        let client = DeepSeekClient::new(&settings.base_url, &settings.effective_api_key());
        let cfg = AgentConfig {
            model: settings.effective_model(),
            temperature: settings.temperature,
            system_prompt: settings.system_prompt.clone(),
            allow_bash: settings.allow_bash,
            workspace_dir: workspace_path.clone(),
            max_tool_rounds: settings.max_tool_rounds.max(1),
            enabled_skills: settings.enabled_skills.clone(),
            skill_roots: skill_roots(&data_dir),
        };
        let result = crate::agent::run_agent(&client, &cfg, history, &text, &event_tx, stop).await;

        // Persist the turn (error turns are persisted with a flag so the UI can restore them).
        let store = SessionStore::new(&data_dir);
        match &result {
            Ok(output) => {
                let _ = store.append_assistant(&task_sid, output, false);
            }
            Err(_) => {
                let empty = crate::agent::AgentOutput {
                    content: String::new(),
                    reasoning: String::new(),
                    tools: Vec::new(),
                    api_messages: Vec::new(),
                };
                let _ = store.append_assistant(&task_sid, &empty, true);
            }
        }
        drop(event_tx);
        let _ = forwarder.await;
        stop_flags.lock().unwrap().remove(&task_sid);
    });

    Ok(json!({ "sessionId": sid, "messageId": message_id }))
}

#[tauri::command]
pub fn chat_stop(state: State<'_, AppState>, id: String) -> AppResult<()> {
    if let Some(flag) = state.stop_flags.lock().unwrap().get(&id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

// ---- skills ----

#[tauri::command]
pub fn skills_list(state: State<'_, AppState>) -> Vec<SkillInfo> {
    let settings = state.settings.read().unwrap().clone();
    skills::scan_skills(&skill_roots(&state.data_dir), &settings.enabled_skills)
}

// ---- models ----

#[tauri::command]
pub fn models_list() -> Vec<Value> {
    vec![
        json!({ "id": "deepseek-v4-flash", "name": "DeepSeek V4 Flash - 快速高性价比" }),
        json!({ "id": "deepseek-v4-pro", "name": "DeepSeek V4 Pro - 更强推理" }),
    ]
}

#[tauri::command]
pub async fn ping_provider(api_key: String, base_url: String) -> AppResult<Value> {
    let client = DeepSeekClient::new(&base_url, &api_key);
    match client.list_models().await {
        Ok(models) => Ok(json!({ "ok": true, "models": models })),
        Err(e) => Ok(json!({ "ok": false, "error": e.to_string() })),
    }
}

// ---- workspaces ----

#[tauri::command]
pub fn workspaces_list(state: State<'_, AppState>) -> crate::workspaces::WorkspaceState {
    state.workspaces.read().unwrap().clone()
}

#[tauri::command]
pub fn workspaces_add(
    state: State<'_, AppState>,
    name: String,
    path: String,
) -> AppResult<crate::workspaces::WorkspaceState> {
    use crate::workspaces::{Workspace, WorkspaceStore};
    let raw = path.trim().to_string();
    if raw.is_empty() {
        return Err(AppError::Config("工作区路径不能为空".into()));
    }
    let canon = std::fs::canonicalize(&raw)
        .map_err(|_| AppError::Config("路径不存在或不是目录".into()))?;
    if !canon.is_dir() {
        return Err(AppError::Config("路径不是目录".into()));
    }
    let canon_str = canon.to_string_lossy().into_owned();
    let name = if name.trim().is_empty() {
        canon
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "工作区".into())
    } else {
        name.trim().to_string()
    };
    let data_dir = state.data_dir.clone();
    let mut ws = state.workspaces.write().unwrap();
    let w = Workspace {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        path: canon_str,
    };
    if ws.current.is_none() {
        ws.current = Some(w.id.clone());
    }
    ws.items.push(w);
    let store = WorkspaceStore::new(&data_dir);
    store.save(&ws)?;
    Ok(ws.clone())
}

#[tauri::command]
pub fn workspaces_remove(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<crate::workspaces::WorkspaceState> {
    use crate::workspaces::WorkspaceStore;
    let data_dir = state.data_dir.clone();
    let mut ws = state.workspaces.write().unwrap();
    ws.items.retain(|w| w.id != id);
    if ws.current.as_deref() == Some(id.as_str()) {
        ws.current = ws.items.first().map(|w| w.id.clone());
    }
    let store = WorkspaceStore::new(&data_dir);
    store.save(&ws)?;
    Ok(ws.clone())
}

#[tauri::command]
pub fn workspaces_set_current(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<crate::workspaces::WorkspaceState> {
    use crate::workspaces::WorkspaceStore;
    let data_dir = state.data_dir.clone();
    let mut ws = state.workspaces.write().unwrap();
    if !ws.items.iter().any(|w| w.id == id) {
        return Err(AppError::Config("工作区不存在".into()));
    }
    ws.current = Some(id);
    let store = WorkspaceStore::new(&data_dir);
    store.save(&ws)?;
    Ok(ws.clone())
}
