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
    session_id: Uuid,
    profile_id: Uuid,
    expires_at: DateTime<Utc>,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn activate_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: Uuid,
    session_id: Option<Uuid>,
    confirmed: Option<bool>,
) -> AppResult<Session> {
    state
        .sessions
        .activate(
            &app,
            &state.profiles,
            profile_id,
            session_id,
            confirmed.unwrap_or(false),
        )
        .await
}

#[tauri::command(rename_all = "snake_case")]
pub fn current_session(
    state: State<'_, AppState>,
    session_id: Uuid,
) -> AppResult<Option<Session>> {
    Ok(state.sessions.current(session_id))
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_sessions(state: State<'_, AppState>) -> AppResult<Vec<Session>> {
    Ok(state.sessions.list())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn deactivate(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: Uuid,
) -> AppResult<()> {
    state.sessions.deactivate(&app, session_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn refresh_token(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: Uuid,
) -> AppResult<RefreshResult> {
    let (profile_id, expires_at) = state.sessions.refresh_token(session_id).await?;
    let _ = app.emit(
        "profile:token_refreshed",
        TokenRefreshed {
            session_id,
            profile_id,
            expires_at,
        },
    );
    Ok(RefreshResult { expires_at })
}
