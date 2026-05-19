//! 에뮬레이터 모드 인증 — 없음(noop).
//!
//! Firestore 에뮬레이터는 인증 헤더를 요구하지 않는다. 호스트 지정은
//! 프로파일의 `firestore_host`/`auth_host`가 담당하며 여기서는 토큰을
//! 공급하지 않는다.

use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use secrecy::SecretString;

use crate::auth::AuthHandle;
use crate::error::AppResult;
use crate::profile::ProfileMode;

#[derive(Debug, Clone, Copy, Default)]
pub struct EmulatorAuth;

impl AuthHandle for EmulatorAuth {
    fn bearer_token(&self) -> BoxFuture<'_, AppResult<Option<SecretString>>> {
        Box::pin(async { Ok(None) })
    }

    fn expires_at(&self) -> Option<DateTime<Utc>> {
        None
    }

    fn mode(&self) -> ProfileMode {
        ProfileMode::Emulator
    }
}
