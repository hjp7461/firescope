//! Tauri 관리 상태 (`app.manage`)와 세션 수명주기.
//!
//! 동시 활성 세션은 **항상 1개**다 (`docs/07-profiles.md` 다중 세션 기본
//! 정책). 프로파일 전환 시 기존 세션을 먼저 해제하고 진행 중 스트림을
//! 모두 취소한 뒤 새 세션을 활성화한다.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::Serialize;
use tauri::{Emitter, Runtime};
use uuid::Uuid;

use crate::adapters::TauriTokenSink;
use crate::auth::{AuthHandle, EmulatorAuth, IdTokenAuth, ServiceAccountAuth};
use crate::error::{AppError, AppResult};
use crate::firestore::FirestoreClient;
use crate::profile::store::ProfileManager;
use crate::profile::{Credential, Profile, ProfileMode};

/// 진행 중인 쿼리 스트림 추적기.
///
/// Phase 2에서 `stream_id → CancellationToken` 맵으로 채워진다. 지금은
/// 세션 해제 흐름의 호출 지점만 확정한다 (`cancel_all`은 아직 no-op).
#[derive(Default)]
pub struct StreamRegistry {
    _private: (),
}

impl StreamRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 활성 세션 해제/전환 시 진행 중 모든 스트림을 취소. (Phase 2에서 구현)
    pub fn cancel_all(&self) {}
}

/// IPC `Session` (`docs/03-ipc-contract.md`). 자격증명 본문은 포함되지 않는다.
#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub session_id: Uuid,
    pub profile_id: Uuid,
    pub profile_name: String,
    pub project_id: String,
    pub mode: ProfileMode,
    pub activated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Clone)]
struct DeactivatedPayload {
    profile_id: Uuid,
}

/// 활성 세션 1개의 런타임 묶음. `Drop`되면 `ServiceAccountAuth`의 토큰
/// 갱신 태스크도 함께 정리된다 (그쪽 `Drop`이 abort).
struct ActiveSession {
    session_id: Uuid,
    profile: Profile,
    #[allow(dead_code)] // Phase 2: 쿼리 시 이 설정으로 FirestoreDb 생성
    firestore: FirestoreClient,
    auth: Arc<dyn AuthHandle>,
    activated_at: DateTime<Utc>,
}

impl ActiveSession {
    fn to_dto(&self) -> Session {
        Session {
            session_id: self.session_id,
            profile_id: self.profile.id,
            profile_name: self.profile.name.clone(),
            project_id: self.profile.project_id.clone(),
            mode: self.profile.mode,
            activated_at: self.activated_at,
            expires_at: self.auth.expires_at(),
        }
    }
}

pub struct SessionManager {
    active: RwLock<Option<ActiveSession>>,
    streams: Arc<StreamRegistry>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            active: RwLock::new(None),
            streams: Arc::new(StreamRegistry::new()),
        }
    }

    pub fn streams(&self) -> &Arc<StreamRegistry> {
        &self.streams
    }

    pub fn current(&self) -> Option<Session> {
        self.active.read().as_ref().map(ActiveSession::to_dto)
    }

    pub fn is_active(&self) -> bool {
        self.active.read().is_some()
    }

    /// 프로파일을 활성화하여 세션을 시작한다.
    ///
    /// 순서가 중요하다: 인증 핸들 구성(서비스 계정은 실제 토큰 왕복)을
    /// **기존 세션을 건드리기 전에** 끝낸다. 실패하면 기존 세션을 그대로
    /// 둔 채 에러를 반환한다(원자성).
    pub async fn activate<R: Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        profiles: &ProfileManager,
        profile_id: Uuid,
        confirmed: bool,
    ) -> AppResult<Session> {
        let profile = profiles.get_profile(profile_id).ok_or_else(|| {
            AppError::profile_not_found(format!("no profile with id {profile_id}"))
        })?;

        if profile.require_confirmation && !confirmed {
            return Err(AppError::ConfirmationRequired {
                message: "this profile requires explicit confirmation to activate".into(),
            });
        }

        // 1) 인증 핸들 구성 — 기존 세션을 건드리기 전에 (실패 시 롤백 불필요).
        let auth = self.build_auth(app, profiles, &profile).await?;
        // 2) 연결 설정 해석 (라이브 FirestoreDb는 Phase 2).
        let firestore = FirestoreClient::connect(&profile)?;

        // 3) 기존 세션 해제 (스트림 취소 + 이벤트). prev drop 시 토큰 태스크 정리.
        let previous = self.active.write().take();
        if let Some(prev) = previous {
            self.streams.cancel_all();
            let _ = app.emit(
                "profile:deactivated",
                DeactivatedPayload {
                    profile_id: prev.profile.id,
                },
            );
            drop(prev);
        }

        // 4) 새 세션 설치.
        let session = ActiveSession {
            session_id: Uuid::new_v4(),
            profile,
            firestore,
            auth,
            activated_at: Utc::now(),
        };
        let dto = session.to_dto();
        *self.active.write() = Some(session);

        tracing::info!(
            target: "session",
            profile_id = %profile_id,
            session_id = %dto.session_id,
            "profile activated"
        );
        let _ = app.emit("profile:activated", dto.clone());
        Ok(dto)
    }

    /// 현재 세션 종료. 진행 중 스트림 취소. 활성 세션이 없어도 성공(idempotent).
    pub fn deactivate<R: Runtime>(&self, app: &tauri::AppHandle<R>) -> AppResult<()> {
        let previous = self.active.write().take();
        if let Some(prev) = previous {
            self.streams.cancel_all();
            let profile_id = prev.profile.id;
            drop(prev);
            tracing::info!(target: "session", profile_id = %profile_id, "profile deactivated");
            let _ = app.emit("profile:deactivated", DeactivatedPayload { profile_id });
        }
        Ok(())
    }

    /// 활성 세션의 토큰을 강제 갱신하고 `(profile_id, 새 만료시각)`을 반환.
    /// 잠금을 await 너머로 들고 가지 않도록 핸들만 꺼낸 뒤 갱신한다.
    pub async fn refresh_token(&self) -> AppResult<(Uuid, DateTime<Utc>)> {
        let handle = {
            let guard = self.active.read();
            guard.as_ref().map(|s| (s.profile.id, Arc::clone(&s.auth)))
        };
        let (profile_id, auth) = handle.ok_or_else(|| AppError::NoSession {
            message: "no active session to refresh".into(),
        })?;
        let expires_at = auth.force_refresh().await?.ok_or_else(|| AppError::Auth {
            message: "active session has no refreshable token".into(),
        })?;
        Ok((profile_id, expires_at))
    }

    /// 모드별 인증 핸들 생성. 자격증명 본문은 여기서 Vault → AuthHandle로만
    /// 흐르고 로그/에러/IPC로 새지 않는다.
    async fn build_auth<R: Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        profiles: &ProfileManager,
        profile: &Profile,
    ) -> AppResult<Arc<dyn AuthHandle>> {
        match profile.mode {
            ProfileMode::Emulator => Ok(Arc::new(EmulatorAuth)),

            ProfileMode::ServiceAccount => {
                let cred = profiles.credential(profile.id)?.ok_or_else(|| {
                    AppError::credential_not_found(
                        "service account profile has no stored credential",
                    )
                })?;
                match cred {
                    Credential::ServiceAccount { json } => {
                        let sink = Arc::new(TauriTokenSink::new(app.clone()));
                        let auth = ServiceAccountAuth::new(&json, sink, profile.id).await?;
                        Ok(Arc::new(auth))
                    }
                    Credential::IdToken { .. } => Err(AppError::credential_invalid(
                        "stored credential kind does not match profile mode (service_account)",
                    )),
                }
            }

            ProfileMode::IdToken => {
                let cred = profiles.credential(profile.id)?.ok_or_else(|| {
                    AppError::credential_not_found("id_token profile has no stored credential")
                })?;
                match cred {
                    Credential::IdToken { token } => Ok(Arc::new(IdTokenAuth::new(token))),
                    Credential::ServiceAccount { .. } => Err(AppError::credential_invalid(
                        "stored credential kind does not match profile mode (id_token)",
                    )),
                }
            }
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 앱 전역 상태. `tauri::State<AppState>`로 커맨드에서 접근한다 (원칙 13).
pub struct AppState {
    pub profiles: ProfileManager,
    pub sessions: SessionManager,
}

impl AppState {
    pub fn new(profiles: ProfileManager) -> Self {
        Self {
            profiles,
            sessions: SessionManager::new(),
        }
    }
}
