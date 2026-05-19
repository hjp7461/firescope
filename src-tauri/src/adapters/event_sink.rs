//! `TokenEventSink`의 Tauri 구현 — 토큰 수명주기를 프론트 이벤트로 emit.

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use uuid::Uuid;

use crate::auth::TokenEventSink;

#[derive(Serialize, Clone)]
struct TokenRefreshed {
    profile_id: Uuid,
    expires_at: DateTime<Utc>,
}

#[derive(Serialize, Clone)]
struct TokenExpired {
    profile_id: Uuid,
}

pub struct TauriTokenSink<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriTokenSink<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> TokenEventSink for TauriTokenSink<R> {
    fn token_refreshed(&self, profile_id: Uuid, expires_at: DateTime<Utc>) {
        let _ = self.app.emit(
            "profile:token_refreshed",
            TokenRefreshed {
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
