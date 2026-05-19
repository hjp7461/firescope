mod auth;
mod commands;
mod error;
mod firestore;
mod profile;
mod state;

use tauri::Manager;
use tauri_plugin_store::StoreExt;

use crate::profile::{CredentialVault, ProfileManager};
use crate::state::AppState;

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
        .invoke_handler(tauri::generate_handler![
            commands::profile::list_profiles,
            commands::profile::get_profile,
            commands::profile::create_profile,
            commands::profile::update_profile,
            commands::profile::delete_profile,
            commands::profile::duplicate_profile,
            commands::profile::set_credential,
            commands::profile::clear_credential,
            commands::profile::test_profile,
            commands::profile::export_profiles,
            commands::profile::import_profiles,
            commands::session::activate_profile,
            commands::session::current_session,
            commands::session::deactivate,
            commands::session::refresh_token,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
