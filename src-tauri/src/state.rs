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
use crate::firestore::{FirestoreClient, ResultSink};
use crate::profile::store::ProfileManager;
use crate::profile::{Credential, Profile, ProfileMode};
use crate::query::history::QueryHistoryManager;

/// 진행 중인 쿼리 스트림 추적기. 각 stream_id에 대해
/// 1) 협조적 취소 플래그 (`AtomicBool`),
/// 2) 결과 누적 sink (`ResultSink`, 임시 NDJSON) — `export_result` IPC에서 소비,
///
/// 를 묶어 보관한다.
///
/// sink는 finished/cancel/sessions deactivate 시 즉시 drop되어 임시 파일이
/// unlink된다 (원칙 5 Secret Lifetime — 운영 데이터를 디스크에 잔존시키지 않음).
#[derive(Default)]
pub struct StreamRegistry {
    inner: parking_lot::Mutex<std::collections::HashMap<String, StreamEntry>>,
}

struct StreamEntry {
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    /// `query_documents` 시작 시 생성, 종료/취소 시 drop. `take_sink`로
    /// 외부에서 빼낼 수도 있으나 통상은 등록자가 보관한다.
    sink: Option<Arc<parking_lot::Mutex<ResultSink>>>,
}

impl StreamRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 스트림 등록 후 (취소 플래그, sink 핸들) 반환.
    /// sink 생성에 실패하면 (예: 임시 디렉터리 쓰기 권한 없음) sink 없이 진행.
    pub fn register(
        &self,
        stream_id: &str,
    ) -> (
        Arc<std::sync::atomic::AtomicBool>,
        Option<Arc<parking_lot::Mutex<ResultSink>>>,
    ) {
        let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sink = match ResultSink::new() {
            Ok(s) => Some(Arc::new(parking_lot::Mutex::new(s))),
            Err(e) => {
                tracing::warn!(
                    target: "query",
                    error = %e,
                    stream_id = %stream_id,
                    "failed to create result sink; export_result will be unavailable"
                );
                None
            }
        };
        let entry = StreamEntry {
            cancelled: Arc::clone(&flag),
            sink: sink.as_ref().map(Arc::clone),
        };
        self.inner.lock().insert(stream_id.to_string(), entry);
        (flag, sink)
    }

    /// 취소되었거나 등록되지 않은(=정리됨) 스트림이면 true.
    pub fn is_cancelled(&self, stream_id: &str) -> bool {
        self.inner
            .lock()
            .get(stream_id)
            .map(|e| e.cancelled.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(true)
    }

    pub fn cancel(&self, stream_id: &str) {
        if let Some(e) = self.inner.lock().get(stream_id) {
            e.cancelled
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// 스트림 완료 — 취소 플래그만 제거. sink는 별도로 export 가능하도록
    /// 등록 유지(다음 쿼리가 시작되면 그때 교체된다).
    pub fn finish(&self, stream_id: &str) {
        if let Some(e) = self.inner.lock().get_mut(stream_id) {
            e.cancelled
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// `export_result` 등에서 사용할 sink 핸들 조회.
    pub fn sink(&self, stream_id: &str) -> Option<Arc<parking_lot::Mutex<ResultSink>>> {
        self.inner
            .lock()
            .get(stream_id)
            .and_then(|e| e.sink.as_ref().map(Arc::clone))
    }

    /// 활성 세션 해제/전환 시 진행 중 모든 스트림 취소 + sink 폐기.
    pub fn cancel_all(&self) {
        let mut guard = self.inner.lock();
        for entry in guard.values() {
            entry
                .cancelled
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        // sink Arc drop → 마지막 참조 해제 시 임시 파일 unlink.
        guard.clear();
    }

    /// 단일 스트림 등록 해제 + sink 폐기 (사용자가 결과 폐기를 명시할 때).
    pub fn drop_stream(&self, stream_id: &str) {
        self.inner.lock().remove(stream_id);
    }
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

        // 1) 자격증명 1회 조회 → 인증 핸들 + 라이브 FirestoreDb 구성.
        //    기존 세션을 건드리기 전에 끝낸다 (실패 시 롤백 불필요).
        let credential = profiles.credential(profile_id)?;
        let auth = self.build_auth(app, &profile, credential.as_ref()).await?;
        let firestore = FirestoreClient::connect(&profile, credential.as_ref()).await?;

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

    /// 활성 세션의 라이브 Firestore 클라이언트 (clone은 값쌈 — 내부 Arc).
    /// 잠금을 await 너머로 들고 가지 않도록 clone해서 반환한다.
    pub fn firestore(&self) -> AppResult<FirestoreClient> {
        self.active
            .read()
            .as_ref()
            .map(|s| s.firestore.clone())
            .ok_or_else(|| AppError::NoSession {
                message: "no active session".into(),
            })
    }

    /// 모드별 인증 핸들 생성. 자격증명 본문은 여기서 Vault → AuthHandle로만
    /// 흐르고 로그/에러/IPC로 새지 않는다.
    async fn build_auth<R: Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        profile: &Profile,
        credential: Option<&Credential>,
    ) -> AppResult<Arc<dyn AuthHandle>> {
        match profile.mode {
            ProfileMode::Emulator => Ok(Arc::new(EmulatorAuth)),

            ProfileMode::ServiceAccount => match credential {
                Some(Credential::ServiceAccount { json }) => {
                    let sink = Arc::new(TauriTokenSink::new(app.clone()));
                    let auth = ServiceAccountAuth::new(json, sink, profile.id).await?;
                    Ok(Arc::new(auth))
                }
                Some(Credential::IdToken { .. }) => Err(AppError::credential_invalid(
                    "stored credential kind does not match profile mode (service_account)",
                )),
                None => Err(AppError::credential_not_found(
                    "service account profile has no stored credential",
                )),
            },

            ProfileMode::IdToken => match credential {
                Some(Credential::IdToken { token }) => {
                    Ok(Arc::new(IdTokenAuth::new(token.clone())))
                }
                Some(Credential::ServiceAccount { .. }) => Err(AppError::credential_invalid(
                    "stored credential kind does not match profile mode (id_token)",
                )),
                None => Err(AppError::credential_not_found(
                    "id_token profile has no stored credential",
                )),
            },
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
    pub history: QueryHistoryManager,
}

impl AppState {
    pub fn new(profiles: ProfileManager, history: QueryHistoryManager) -> Self {
        Self {
            profiles,
            sessions: SessionManager::new(),
            history,
        }
    }
}
