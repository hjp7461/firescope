mod adapters;
mod auth;
mod commands;
mod error;
mod firestore;
mod profile;
mod query;
mod state;

use std::sync::Arc;

use tauri::Manager;
use tauri_plugin_store::StoreExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use crate::adapters::log_layer::LogLayer;
use crate::adapters::TauriProfileRepository;
use crate::profile::{OsKeyringVault, ProfileManager};
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            // tracing 구독자 초기화 (원칙 11: tracing only, 원칙 3: 패닉 금지).
            let filter = EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,firescope_lib=debug"));
            if tracing_subscriber::registry()
                .with(filter)
                .with(LogLayer::new(app.handle().clone()))
                .try_init()
                .is_err()
            {
                // 이미 초기화됐거나 실패 — 패닉 금지(원칙 3), 앱은 계속.
            }

            // 어댑터 합성 (원칙 13: AppState가 단일 진입점).
            let store = app.store("profiles.json")?;
            let repo = Arc::new(TauriProfileRepository::new(store));
            let vault = Arc::new(OsKeyringVault::new());
            let profiles = ProfileManager::new(repo, vault)?;
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
            commands::query::list_collections,
            commands::query::list_subcollections,
            commands::query::get_document,
            commands::query::query_documents,
            commands::query::cancel_stream,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
