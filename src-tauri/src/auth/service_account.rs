//! 서비스 계정 모드 — `gcp_auth`로 액세스 토큰 발급 + 자동 갱신.
//!
//! 원칙 4: 순수 도메인 — `tauri::*`를 import하지 않는다. 토큰 수명주기
//! 알림은 [`TokenEventSink`] 포트로만 보낸다.
//!
//! 보안: `gcp_auth::Error`의 `Display`는 내부 `serde_json::Error`를 통해
//! 입력(서비스 계정 JSON, 즉 private_key)을 일부 노출할 수 있다. 따라서
//! gcp_auth 에러는 **절대 그대로 전파하지 않고** 일반 메시지로 치환한다.
//! 토큰 문자열은 로그/에러/IPC 어디에도 출력하지 않는다.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use gcp_auth::{CustomServiceAccount, TokenProvider};
use parking_lot::RwLock;
use secrecy::{ExposeSecret, SecretString};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::auth::{AuthHandle, TokenEventSink, FIRESTORE_SCOPES};
use crate::error::{AppError, AppResult};
use crate::profile::ProfileMode;

/// 만료 몇 분 전에 미리 갱신할지.
const REFRESH_LEAD: chrono::TimeDelta = chrono::TimeDelta::minutes(5);
/// 갱신 스케줄의 하한 (만료가 임박/과거여도 busy-loop 방지).
const MIN_SLEEP_SECS: i64 = 5;
/// 갱신 실패 시 재시도 간격.
const RETRY_BACKOFF_SECS: u64 = 30;

#[derive(Clone)]
struct CachedToken {
    token: SecretString,
    expires_at: DateTime<Utc>,
}

pub struct ServiceAccountAuth {
    provider: Arc<CustomServiceAccount>,
    cache: Arc<RwLock<CachedToken>>,
    refresh_task: JoinHandle<()>,
}

impl ServiceAccountAuth {
    /// 서비스 계정 JSON으로 초기화. **즉시 1회 토큰을 발급**하여
    /// 자격증명의 실제 유효성을 검증하고(잘못된 키면 여기서 실패),
    /// 이후 만료 5분 전 자동 갱신 태스크를 띄운다.
    pub async fn new(
        account_json: &SecretString,
        sink: Arc<dyn TokenEventSink>,
        profile_id: Uuid,
    ) -> AppResult<Self> {
        let provider = Self::provider_from(account_json)?;
        let initial = Self::fetch(&provider).await?;
        let cache = Arc::new(RwLock::new(initial));

        let refresh_task = tokio::spawn(Self::refresh_loop(
            Arc::clone(&provider),
            Arc::clone(&cache),
            sink,
            profile_id,
        ));

        Ok(Self {
            provider,
            cache,
            refresh_task,
        })
    }

    /// 백그라운드 태스크/sink 없이 자격증명만 1회 검증한다 (test_profile용).
    /// 성공 시 토큰 만료 시각 반환.
    pub async fn validate(account_json: &SecretString) -> AppResult<DateTime<Utc>> {
        let provider = Self::provider_from(account_json)?;
        Ok(Self::fetch(&provider).await?.expires_at)
    }

    fn provider_from(account_json: &SecretString) -> AppResult<Arc<CustomServiceAccount>> {
        // expose는 이 호출에만 한정. 파싱 실패 메시지에 본문이 새지 않도록
        // gcp_auth 에러는 통째로 폐기하고 일반 메시지로 치환한다.
        let provider =
            CustomServiceAccount::from_json(account_json.expose_secret()).map_err(|_| {
                AppError::Auth {
                    message: "failed to load service account credentials".into(),
                }
            })?;
        Ok(Arc::new(provider))
    }

    /// 토큰 1회 발급. 토큰 문자열은 메시지/로그로 새지 않는다.
    async fn fetch(provider: &CustomServiceAccount) -> AppResult<CachedToken> {
        let token = provider
            .token(FIRESTORE_SCOPES)
            .await
            .map_err(|_| AppError::Auth {
                message: "failed to obtain access token".into(),
            })?;
        Ok(CachedToken {
            token: SecretString::from(token.as_str().to_owned()),
            expires_at: token.expires_at(),
        })
    }

    /// 다음 갱신까지 대기할 시간. `expires_at - 5분`을 목표로 하되 하한 적용.
    fn sleep_until_refresh(expires_at: DateTime<Utc>) -> std::time::Duration {
        let target = expires_at - REFRESH_LEAD;
        let secs = (target - Utc::now()).num_seconds().max(MIN_SLEEP_SECS);
        std::time::Duration::from_secs(secs as u64)
    }

    async fn refresh_loop(
        provider: Arc<CustomServiceAccount>,
        cache: Arc<RwLock<CachedToken>>,
        sink: Arc<dyn TokenEventSink>,
        profile_id: Uuid,
    ) {
        loop {
            let expires_at = cache.read().expires_at;
            tokio::time::sleep(Self::sleep_until_refresh(expires_at)).await;

            match Self::fetch(&provider).await {
                Ok(fresh) => {
                    let expires_at = fresh.expires_at;
                    *cache.write() = fresh;
                    tracing::info!(
                        target: "auth",
                        profile_id = %profile_id,
                        "service account token refreshed"
                    );
                    sink.token_refreshed(profile_id, expires_at);
                }
                Err(_) => {
                    // 에러 본문은 이미 일반화됨. 토큰/키는 어디에도 없음.
                    tracing::warn!(
                        target: "auth",
                        profile_id = %profile_id,
                        "token refresh failed; will retry"
                    );
                    sink.token_expired(profile_id);
                    tokio::time::sleep(std::time::Duration::from_secs(RETRY_BACKOFF_SECS)).await;
                }
            }
        }
    }
}

/// 세션 종료 시 백그라운드 갱신 태스크를 확실히 정리한다.
impl Drop for ServiceAccountAuth {
    fn drop(&mut self) {
        self.refresh_task.abort();
    }
}

impl AuthHandle for ServiceAccountAuth {
    fn bearer_token(&self) -> BoxFuture<'_, AppResult<Option<SecretString>>> {
        let cache = Arc::clone(&self.cache);
        Box::pin(async move {
            // 정상 흐름에서는 자동 갱신 덕에 항상 유효하다. 방어적으로만 체크.
            let valid = {
                let c = cache.read();
                if Utc::now() < c.expires_at - chrono::TimeDelta::seconds(20) {
                    Some(c.token.clone())
                } else {
                    None
                }
            };
            match valid {
                Some(token) => Ok(Some(token)),
                None => Err(AppError::Auth {
                    message: "access token expired and not yet refreshed".into(),
                }),
            }
        })
    }

    fn expires_at(&self) -> Option<DateTime<Utc>> {
        Some(self.cache.read().expires_at)
    }

    fn mode(&self) -> ProfileMode {
        ProfileMode::ServiceAccount
    }

    /// 즉시 재발급하여 캐시를 교체하고 새 만료 시각을 반환한다.
    /// 백그라운드 루프는 다음 주기에 새 `expires_at`을 읽어 자동 보정된다.
    fn force_refresh(&self) -> BoxFuture<'_, AppResult<Option<DateTime<Utc>>>> {
        let provider = Arc::clone(&self.provider);
        let cache = Arc::clone(&self.cache);
        Box::pin(async move {
            let fresh = Self::fetch(&provider).await?;
            let expires_at = fresh.expires_at;
            *cache.write() = fresh;
            Ok(Some(expires_at))
        })
    }
}
