//! `TokenEventSink`의 Tauri 구현 — 토큰 수명주기를 프론트 이벤트로 emit.

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use uuid::Uuid;

use crate::auth::TokenEventSink;

#[derive(Serialize, Clone)]
struct TokenRefreshed {
    session_id: Uuid,
    profile_id: Uuid,
    expires_at: DateTime<Utc>,
}

#[derive(Serialize, Clone)]
struct TokenExpired {
    profile_id: Uuid,
}

pub struct TauriTokenSink<R: Runtime> {
    app: AppHandle<R>,
    session_id: Uuid,
}

impl<R: Runtime> TauriTokenSink<R> {
    pub fn new(app: AppHandle<R>, session_id: Uuid) -> Self {
        Self { app, session_id }
    }
}

impl<R: Runtime> TokenEventSink for TauriTokenSink<R> {
    fn token_refreshed(&self, profile_id: Uuid, expires_at: DateTime<Utc>) {
        let _ = self.app.emit(
            "profile:token_refreshed",
            TokenRefreshed {
                session_id: self.session_id,
                profile_id,
                expires_at,
            },
        );
    }

    fn token_expired(&self, profile_id: Uuid) {
        let _ = self
            .app
            .emit("profile:token_expired", TokenExpired { profile_id });
    }
}
