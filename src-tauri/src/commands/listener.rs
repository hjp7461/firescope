//! Realtime 리스너 커맨드 (`docs/03-ipc-contract.md` §8.5, Phase 11).
//!
//! 활성 세션의 라이브 `FirestoreDb`를 사용해 `db.create_listener` 기반
//! 리스너를 등록·종료한다. 세션 deactivate 시 자동 정리되므로 여기서는
//! lifecycle을 직접 다루지 않는다.

use std::sync::Arc;

use serde::Deserialize;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::error::AppResult;
use crate::firestore::listener::{self, ListenerInfoDto};
use crate::query::dsl::ListenerDsl;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct StartListenerParams {
    pub listener_id: String,
    pub dsl: ListenerDsl,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn start_listener(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: Uuid,
    params: StartListenerParams,
) -> AppResult<()> {
    let client = state.sessions.firestore(session_id)?;
    let registry = Arc::clone(state.sessions.listeners());
    listener::start_listener(
        app,
        client.db.clone(),
        registry,
        params.listener_id,
        session_id,
        params.dsl,
    )
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn stop_listener(
    state: State<'_, AppState>,
    listener_id: String,
) -> AppResult<()> {
    let registry = Arc::clone(state.sessions.listeners());
    listener::stop_listener(registry, &listener_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_listeners(state: State<'_, AppState>) -> AppResult<Vec<ListenerInfoDto>> {
    Ok(state.sessions.listeners().list())
}
