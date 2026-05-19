mod auth;
mod error;
mod profile;
mod state;

use tauri::Manager;
use tauri_plugin_store::StoreExt;

use crate::profile::{CredentialVault, ProfileManager};
use crate::state::AppState;

// Phase 0 IPC 동작 확인용. Phase 1-D에서 프로파일 커맨드로 대체된다.
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            // profiles.json: tauri-plugin-store가 앱 데이터 디렉토리에 보관.
            let store = app.store("profiles.json")?;
            let profiles = ProfileManager::load(store, CredentialVault::new())?;
            app.manage(AppState::new(profiles));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
