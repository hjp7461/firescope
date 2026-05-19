//! 세션 커맨드 (`docs/03-ipc-contract.md` §2).

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::error::AppResult;
use crate::state::{AppState, Session};

#[derive(Serialize)]
pub struct RefreshResult {
    pub expires_at: DateTime<Utc>,
}

#[derive(Serialize, Clone)]
struct TokenRefreshed {
    profile_id: Uuid,
    expires_at: DateTime<Utc>,
}

#[tauri::command]
pub async fn activate_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: Uuid,
    confirmed: Option<bool>,
) -> AppResult<Session> {
    state
        .sessions
        .activate(
            &app,
            &state.profiles,
            profile_id,
            confirmed.unwrap_or(false),
        )
        .await
}

#[tauri::command]
pub fn current_session(state: State<'_, AppState>) -> AppResult<Option<Session>> {
    Ok(state.sessions.current())
}

#[tauri::command]
pub fn deactivate(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    state.sessions.deactivate(&app)
}

#[tauri::command]
pub async fn refresh_token(app: AppHandle, state: State<'_, AppState>) -> AppResult<RefreshResult> {
    let (profile_id, expires_at) = state.sessions.refresh_token().await?;
    let _ = app.emit(
        "profile:token_refreshed",
        TokenRefreshed {
            profile_id,
            expires_at,
        },
    );
    Ok(RefreshResult { expires_at })
}
