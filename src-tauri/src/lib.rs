pub mod agent;
mod commands;
pub mod deepseek;
mod error;
mod secrets;
pub mod sessions;
pub mod settings;
pub mod skills;
mod tools;
pub mod workspaces;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

pub use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager;
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = settings::data_dir();
            std::fs::create_dir_all(&data_dir).ok();
            skills::seed_skills(&data_dir);
            let settings = settings::SettingsStore::new(&data_dir).load();
            let workspaces =
                workspaces::seed_from_settings(&data_dir, &settings.workspace_dir);
            let state = AppState {
                data_dir: data_dir.clone(),
                settings: RwLock::new(settings),
                sessions: Mutex::new(sessions::SessionStore::new(&data_dir)),
                workspaces: RwLock::new(workspaces),
                stop_flags: Arc::new(Mutex::new(HashMap::new())),
            };
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::chat_send,
            commands::chat_stop,
            commands::sessions_list,
            commands::session_create,
            commands::session_delete,
            commands::session_messages,
            commands::settings_get,
            commands::settings_set,
            commands::skills_list,
            commands::models_list,
            commands::ping_provider,
            commands::workspaces_list,
            commands::workspaces_add,
            commands::workspaces_remove,
            commands::workspaces_set_current,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
