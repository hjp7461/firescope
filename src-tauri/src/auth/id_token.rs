//! ID 토큰 모드 — 사용자가 제공한 토큰을 헤더에 그대로 주입.
//!
//! 갱신 없음: 만료되면 사용자가 새 토큰을 다시 입력해야 한다
//! (`docs/02-architecture.md` 인증 모델). 형식 검증은 저장 시점
//! (`profile::validation`)에서 끝났고, 서명/만료 검증은 의도적으로
//! 후속 단계로 미룬다 — 이 모듈의 책임은 "헤더 주입"뿐이다.

use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use secrecy::SecretString;

use crate::auth::AuthHandle;
use crate::error::AppResult;
use crate::profile::ProfileMode;

pub struct IdTokenAuth {
    token: SecretString,
}

impl IdTokenAuth {
    pub fn new(token: SecretString) -> Self {
        Self { token }
    }
}

impl AuthHandle for IdTokenAuth {
    fn bearer_token(&self) -> BoxFuture<'_, AppResult<Option<SecretString>>> {
        let token = self.token.clone();
        Box::pin(async move { Ok(Some(token)) })
    }

    fn expires_at(&self) -> Option<DateTime<Utc>> {
        None
    }

    fn mode(&self) -> ProfileMode {
        ProfileMode::IdToken
    }
}
