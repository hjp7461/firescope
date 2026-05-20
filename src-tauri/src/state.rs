//! Tauri 관리 상태 (`app.manage`)와 세션 수명주기.
//!
//! 동시 활성 세션은 **N개**다 (`docs/07-profiles.md` 멀티 세션 정책,
//! IPC v0.8). 세션은 `HashMap<session_id, ActiveSession>`로 보관되며,
//! 각 세션마다 자체 인증/Firestore 연결을 가진다. 세션 종료 시 해당
//! 세션의 진행 중 스트림만 취소된다.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::Serialize;
use tauri::{Emitter, Runtime};
use uuid::Uuid;

/// 멀티탭 세션 소프트캡. 초과해도 거부하지 않고 `session:limit_warning` 이벤트만 emit.
const SESSION_SOFT_CAP: usize = 10;

use crate::adapters::TauriTokenSink;
use crate::auth::{AuthHandle, EmulatorAuth, IdTokenAuth, ServiceAccountAuth};
use crate::error::{AppError, AppResult};
use crate::firestore::{FirestoreClient, ListenerRegistry, ResultSink};
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
    session_id: uuid::Uuid,
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
        session_id: uuid::Uuid,
    ) -> (
        Arc<std::sync::atomic::AtomicBool>,
        Option<Arc<parking_lot::Mutex<ResultSink>>>,
    ) {
        let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sink = match ResultSink::new() {
            Ok(s) => {
                // sink path를 debug 레벨로 노출 — 수동 lifecycle 검증용
                // (cancel_all/Drop 시점에 ls로 unlink 확인 가능).
                tracing::debug!(
                    target: "query",
                    stream_id = %stream_id,
                    session_id = %session_id,
                    sink_path = %s.path().display(),
                    "result sink created"
                );
                Some(Arc::new(parking_lot::Mutex::new(s)))
            }
            Err(e) => {
                tracing::warn!(
                    target: "query",
                    error = %e,
                    stream_id = %stream_id,
                    session_id = %session_id,
                    "failed to create result sink; export_result will be unavailable"
                );
                None
            }
        };
        let entry = StreamEntry {
            session_id,
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

    /// 그 세션이 보유한 모든 스트림을 취소·정리 (다른 세션은 무관).
    pub fn cancel_session(&self, session_id: uuid::Uuid) {
        self.inner.lock().retain(|_, e| {
            if e.session_id == session_id {
                e.cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
                false // drop the entry → Arc drop → sink unlinks file
            } else {
                true
            }
        });
    }

    /// 단일 스트림 등록 해제 + sink 폐기 (사용자가 결과 폐기를 명시할 때).
    pub fn drop_stream(&self, stream_id: &str) {
        self.inner.lock().remove(stream_id);
    }
}

#[cfg(test)]
mod registry_tests {
    //! Sink lifecycle 통합 검증: ResultSink::Drop이 unlink하는 것은
    //! sink 단위 테스트가 커버한다. 여기서는 *레지스트리 레벨*에서
    //! Arc 수명 관리가 올바른지 — 즉 cancel_all/drop_stream/replacement
    //! 시점에 실제 임시 파일이 사라지는지 — 를 확인한다 (원칙 5).
    use super::*;

    fn sink_path(arc: &Arc<parking_lot::Mutex<crate::firestore::ResultSink>>) -> std::path::PathBuf {
        arc.lock().path().to_path_buf()
    }

    #[test]
    fn register_creates_sink_file_on_disk() {
        let r = StreamRegistry::new();
        let sid = uuid::Uuid::new_v4();
        let (_flag, sink) = r.register("a", sid);
        let sink = sink.expect("sink should be created in normal environment");
        let path = sink_path(&sink);
        assert!(path.exists(), "sink file must exist after register");
    }

    #[test]
    fn cancel_all_unlinks_all_sink_files_when_external_refs_drop() {
        let r = StreamRegistry::new();
        let sid = uuid::Uuid::new_v4();
        let (_, sink_a) = r.register("a", sid);
        let (_, sink_b) = r.register("b", sid);
        let path_a = sink_path(sink_a.as_ref().unwrap());
        let path_b = sink_path(sink_b.as_ref().unwrap());
        assert!(path_a.exists());
        assert!(path_b.exists());

        // 외부(=command)가 보유한 Arc 해제 — 실제 큐에서는 streaming task가
        // 끝나면서 자연스럽게 drop된다.
        drop(sink_a);
        drop(sink_b);
        // registry 내부 Arc 해제 — 마지막 참조가 사라지면서 ResultSink::Drop 실행.
        r.cancel_all();

        assert!(!path_a.exists(), "cancel_all must unlink sink_a");
        assert!(!path_b.exists(), "cancel_all must unlink sink_b");
    }

    #[test]
    fn drop_stream_unlinks_only_that_sink() {
        let r = StreamRegistry::new();
        let sid = uuid::Uuid::new_v4();
        let (_, sink_a) = r.register("a", sid);
        let (_, sink_b) = r.register("b", sid);
        let path_a = sink_path(sink_a.as_ref().unwrap());
        let path_b = sink_path(sink_b.as_ref().unwrap());

        drop(sink_a);
        r.drop_stream("a");

        assert!(!path_a.exists(), "drop_stream(a) must unlink sink_a");
        assert!(path_b.exists(), "sink_b must remain");
        // 정리
        drop(sink_b);
        r.cancel_all();
    }

    #[test]
    fn cancel_only_does_not_unlink_sink() {
        // export 가능성을 위해 cancel 자체로는 sink가 남아야 한다
        // (사용자가 명시적으로 cancel_stream을 호출한 경우는 commands 계층에서
        // cancel + drop_stream을 함께 부른다 — registry 단독 행위가 아님).
        let r = StreamRegistry::new();
        let sid = uuid::Uuid::new_v4();
        let (_, sink) = r.register("a", sid);
        let path = sink_path(sink.as_ref().unwrap());

        r.cancel("a");
        assert!(path.exists(), "cancel alone must keep sink available for export");
        assert!(r.is_cancelled("a"));
        assert!(r.sink("a").is_some(), "sink handle must still be retrievable");

        // 정리
        drop(sink);
        r.cancel_all();
        assert!(!path.exists());
    }

    #[test]
    fn re_register_same_id_replaces_previous_sink() {
        // commands/query.rs::query_documents는 새 쿼리 시작 시 cancel_all로
        // 일괄 정리하지만, 만에 하나 같은 stream_id로 register가 두 번
        // 불려도 이전 sink가 잔존하지 않아야 한다.
        let r = StreamRegistry::new();
        let sid = uuid::Uuid::new_v4();
        let (_, sink1) = r.register("same", sid);
        let path1 = sink_path(sink1.as_ref().unwrap());
        drop(sink1);

        let (_, sink2) = r.register("same", sid);
        let path2 = sink_path(sink2.as_ref().unwrap());
        assert_ne!(path1, path2, "second register must allocate a new sink file");
        assert!(!path1.exists(), "previous sink must be unlinked when entry is replaced");
        assert!(path2.exists());

        drop(sink2);
        r.cancel_all();
    }

    #[test]
    fn cancel_session_only_cancels_that_sessions_streams() {
        let r = StreamRegistry::new();
        let s1 = uuid::Uuid::new_v4();
        let s2 = uuid::Uuid::new_v4();

        let (flag_a, sink_a) = r.register("a", s1);
        let (flag_b, sink_b) = r.register("b", s2);
        let path_a = sink_path(sink_a.as_ref().unwrap());
        let path_b = sink_path(sink_b.as_ref().unwrap());

        drop(sink_a);
        drop(sink_b);

        r.cancel_session(s1);

        assert!(flag_a.load(std::sync::atomic::Ordering::Relaxed));
        assert!(!flag_b.load(std::sync::atomic::Ordering::Relaxed));
        assert!(!path_a.exists(), "s1's sink must be unlinked");
        assert!(path_b.exists(), "s2's sink must survive");

        // cleanup
        r.cancel_all();
        assert!(!path_b.exists());
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
    session_id: Uuid,
    profile_id: Uuid,
}

#[derive(Serialize, Clone)]
struct SessionLimitWarning {
    active: usize,
    max: usize,
}

/// 활성 세션 1개의 런타임 묶음. `Drop`되면 `ServiceAccountAuth`의 토큰
/// 갱신 태스크도 함께 정리된다 (그쪽 `Drop`이 abort).
///
/// `firestore`는 멀티 세션 N-맵 도입 시 단위 테스트(네트워크 미접속)에서
/// `None`으로 만들 수 있도록 `Option`이다. 프로덕션 경로(`activate`)는
/// 반드시 `Some`을 채워 넣고, `SessionManager::firestore(session_id)`는
/// `None`이면 `AppError::Internal`을 반환해 호출부가 알아챌 수 있게 한다.
pub(super) struct ActiveSession {
    pub(super) session_id: Uuid,
    pub(super) profile: Profile,
    pub(super) firestore: Option<FirestoreClient>,
    pub(super) auth: Arc<dyn AuthHandle>,
    pub(super) activated_at: DateTime<Utc>,
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

/// N개의 활성 세션을 `session_id` 기준 `HashMap`으로 관리한다.
/// 각 세션은 자체 `ActiveSession`(프로파일·인증·Firestore 연결)을 가진다.
/// 멀티 탭 백엔드의 코어 (`docs/superpowers/specs/2026-05-20-multi-tab-design.md` §3.1).
pub struct SessionManager {
    pub(super) sessions: RwLock<std::collections::HashMap<Uuid, ActiveSession>>,
    streams: Arc<StreamRegistry>,
    listeners: Arc<ListenerRegistry>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(std::collections::HashMap::new()),
            streams: Arc::new(StreamRegistry::new()),
            listeners: Arc::new(ListenerRegistry::new()),
        }
    }

    pub fn streams(&self) -> &Arc<StreamRegistry> {
        &self.streams
    }

    pub fn listeners(&self) -> &Arc<ListenerRegistry> {
        &self.listeners
    }

    /// 현재 활성 세션 수.
    pub fn session_count(&self) -> usize {
        self.sessions.read().len()
    }

    /// 지정 세션의 DTO(`Session`). 없으면 `None`.
    pub fn current(&self, session_id: Uuid) -> Option<Session> {
        self.sessions
            .read()
            .get(&session_id)
            .map(ActiveSession::to_dto)
    }

    /// 모든 활성 세션의 DTO 목록.
    pub fn list(&self) -> Vec<Session> {
        self.sessions
            .read()
            .values()
            .map(ActiveSession::to_dto)
            .collect()
    }

    /// 지정 세션의 라이브 Firestore 클라이언트 (clone은 값쌈 — 내부 Arc).
    /// 잠금을 await 너머로 들고 가지 않도록 clone해서 반환한다.
    pub fn firestore(&self, session_id: Uuid) -> AppResult<FirestoreClient> {
        let guard = self.sessions.read();
        let session = guard.get(&session_id).ok_or_else(|| {
            AppError::session_not_found(session_id, "no active session for that id")
        })?;
        match &session.firestore {
            Some(client) => Ok(client.clone()),
            // 테스트 스텁(`fake_session`)으로 만들어진 세션 — 프로덕션 코드 경로에서는
            // 절대 발생하지 않아야 하므로 Internal로 표시한다.
            None => Err(AppError::internal(
                "test stub: no firestore client attached to session",
            )),
        }
    }

    /// 지정 세션의 토큰을 강제 갱신하고 `(profile_id, 새 만료시각)`을 반환.
    /// 잠금을 await 너머로 들고 가지 않도록 핸들만 꺼낸 뒤 갱신한다.
    pub async fn refresh_token(
        &self,
        session_id: Uuid,
    ) -> AppResult<(Uuid, DateTime<Utc>)> {
        let handle = {
            let guard = self.sessions.read();
            guard
                .get(&session_id)
                .map(|s| (s.profile.id, Arc::clone(&s.auth)))
        };
        let (profile_id, auth) = handle.ok_or_else(|| {
            AppError::session_not_found(session_id, "no active session to refresh")
        })?;
        let expires_at = auth.force_refresh().await?.ok_or_else(|| AppError::Auth {
            message: "active session has no refreshable token".into(),
        })?;
        Ok((profile_id, expires_at))
    }

    /// 직접적 맵 제거 — 테스트와 `deactivate*` 흐름 내부에서 사용한다.
    /// 프로덕션 경로는 `deactivate`를 호출해 이벤트까지 같이 발행해야 한다.
    pub(super) fn remove_session(&self, session_id: Uuid) -> Option<ActiveSession> {
        self.sessions.write().remove(&session_id)
    }

    /// 모든 세션 맵을 비운다 — 테스트와 `deactivate_all` 내부 보조 함수.
    pub(super) fn remove_all_sessions(&self) {
        self.sessions.write().clear();
    }

    /// 프로파일을 활성화하여 새 세션을 시작하거나, 지정된 세션을 새 프로파일로
    /// 교체한다 (탭 단위 활성화).
    ///
    /// - `session_id: None` → 새 세션을 생성 (새 탭).
    /// - `session_id: Some(existing)` → 그 세션(탭)을 새 프로파일로 대체.
    ///   그 세션이 보유하던 스트림만 취소되고, 다른 세션은 영향받지 않는다.
    ///
    /// 순서가 중요하다: 인증 핸들 구성(서비스 계정은 실제 토큰 왕복)을
    /// **기존 세션을 건드리기 전에** 끝낸다. 실패하면 기존 세션을 그대로
    /// 둔 채 에러를 반환한다(원자성).
    pub async fn activate<R: Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        profiles: &ProfileManager,
        profile_id: Uuid,
        session_id: Option<Uuid>,
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

        // 1) 세션 ID 확정: 교체이면 기존 UUID 재사용, 신규이면 새 UUID 생성.
        let new_session_id = session_id.unwrap_or_else(Uuid::new_v4);

        // 2) 자격증명 1회 조회 → 인증 핸들 + 라이브 FirestoreDb 구성.
        //    기존 세션을 건드리기 전에 끝낸다 (실패 시 롤백 불필요).
        let credential = profiles.credential(profile_id)?;
        let auth = self
            .build_auth(app, &profile, credential.as_ref(), new_session_id)
            .await?;
        let firestore = FirestoreClient::connect(&profile, credential.as_ref()).await?;

        // 3) 교체 대상이 있으면 그 세션의 스트림과 listener를 정리.
        if let Some(existing) = session_id {
            self.streams.cancel_session(existing);
            self.listeners.shutdown_session(existing).await;
            if let Some(prev) = self.remove_session(existing) {
                let prev_profile_id = prev.profile.id;
                drop(prev);
                let _ = app.emit(
                    "profile:deactivated",
                    DeactivatedPayload {
                        session_id: existing,
                        profile_id: prev_profile_id,
                    },
                );
            }
        }

        // 4) 새 세션 설치.
        let session = ActiveSession {
            session_id: new_session_id,
            profile,
            firestore: Some(firestore),
            auth,
            activated_at: Utc::now(),
        };
        let dto = session.to_dto();
        self.sessions.write().insert(new_session_id, session);

        // 5) 소프트캡 안내 (활성화는 진행).
        let count = self.session_count();
        if count > SESSION_SOFT_CAP {
            let _ = app.emit(
                "session:limit_warning",
                SessionLimitWarning {
                    active: count,
                    max: SESSION_SOFT_CAP,
                },
            );
        }

        tracing::info!(
            target: "session",
            profile_id = %profile_id,
            session_id = %new_session_id,
            active_count = count,
            "profile activated"
        );
        let _ = app.emit("profile:activated", dto.clone());
        Ok(dto)
    }

    /// 지정 세션을 종료한다. 그 세션이 가진 진행 중 스트림과 realtime
    /// listener가 모두 정리된다. 다른 세션 / 알 수 없는 세션 id에 대해서는
    /// idempotent.
    pub async fn deactivate<R: Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        session_id: Uuid,
    ) -> AppResult<()> {
        self.streams.cancel_session(session_id);
        self.listeners.shutdown_session(session_id).await;
        if let Some(prev) = self.remove_session(session_id) {
            let profile_id = prev.profile.id;
            drop(prev);
            tracing::info!(
                target: "session",
                session_id = %session_id,
                profile_id = %profile_id,
                "session deactivated"
            );
            let _ = app.emit(
                "profile:deactivated",
                DeactivatedPayload {
                    session_id,
                    profile_id,
                },
            );
        }
        Ok(())
    }

    /// 모든 활성 세션을 종료한다. 윈도우 종료 / 앱 종료 직전에 호출.
    /// 각 세션마다 `profile:deactivated` 이벤트가 발행된다.
    pub async fn deactivate_all<R: Runtime>(&self, app: &tauri::AppHandle<R>) {
        // 이벤트를 세션별로 발행하기 위해 drain.
        let drained: Vec<(Uuid, Uuid)> = {
            let mut guard = self.sessions.write();
            guard
                .drain()
                .map(|(sid, s)| (sid, s.profile.id))
                .collect()
        };
        self.streams.cancel_all();
        self.listeners.shutdown_all().await;
        for (sid, pid) in drained {
            tracing::info!(
                target: "session",
                session_id = %sid,
                profile_id = %pid,
                "session deactivated (deactivate_all)"
            );
            let _ = app.emit(
                "profile:deactivated",
                DeactivatedPayload {
                    session_id: sid,
                    profile_id: pid,
                },
            );
        }
    }

    /// 모드별 인증 핸들 생성. 자격증명 본문은 여기서 Vault → AuthHandle로만
    /// 흐르고 로그/에러/IPC로 새지 않는다.
    async fn build_auth<R: Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        profile: &Profile,
        credential: Option<&Credential>,
        session_id: Uuid,
    ) -> AppResult<Arc<dyn AuthHandle>> {
        match profile.mode {
            ProfileMode::Emulator => Ok(Arc::new(EmulatorAuth)),

            ProfileMode::ServiceAccount => match credential {
                Some(Credential::ServiceAccount { json }) => {
                    let sink = Arc::new(TauriTokenSink::new(app.clone(), session_id));
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

#[cfg(test)]
mod session_manager_tests {
    //! `SessionManager`의 다중 세션 거동.
    //!
    //! 실제 `activate`는 `AppHandle`과 토큰 발급이 필요해 단위테스트에서
    //! 못 만지지만, 세션 맵 자체와 `remove_session*`은 검증 가능하다.
    //! `ActiveSession.firestore`는 `Option`이라 테스트는 `None`으로 둔다.
    use super::*;

    fn fake_session(profile_id: Uuid) -> ActiveSession {
        ActiveSession {
            session_id: Uuid::new_v4(),
            profile: Profile {
                id: profile_id,
                name: "test".into(),
                description: None,
                project_id: "demo".into(),
                mode: ProfileMode::Emulator,
                color: None,
                tags: Vec::new(),
                group: None,
                firestore_host: Some("localhost:8080".into()),
                auth_host: None,
                require_confirmation: false,
                read_only_warning: false,
                credential_ref: None,
                created_at: Utc::now(),
                last_used_at: None,
                use_count: 0,
            },
            firestore: None,
            auth: Arc::new(EmulatorAuth),
            activated_at: Utc::now(),
        }
    }

    #[test]
    fn list_starts_empty() {
        let m = SessionManager::new();
        assert!(m.list().is_empty());
        assert_eq!(m.session_count(), 0);
    }

    #[test]
    fn insert_then_get_returns_dto() {
        let m = SessionManager::new();
        let sess = fake_session(Uuid::new_v4());
        let sid = sess.session_id;
        m.sessions.write().insert(sid, sess);

        let dto = m.current(sid).expect("session present");
        assert_eq!(dto.session_id, sid);
        assert_eq!(m.session_count(), 1);
        assert_eq!(m.list().len(), 1);
    }

    #[test]
    fn current_returns_none_for_unknown_session() {
        let m = SessionManager::new();
        assert!(m.current(Uuid::new_v4()).is_none());
    }

    #[test]
    fn remove_session_unknown_id_is_idempotent() {
        let m = SessionManager::new();
        let removed = m.remove_session(Uuid::new_v4());
        assert!(removed.is_none());
        assert_eq!(m.session_count(), 0);
    }

    #[test]
    fn deactivate_removes_one_session_only() {
        let m = SessionManager::new();
        let s1 = fake_session(Uuid::new_v4());
        let s2 = fake_session(Uuid::new_v4());
        let sid1 = s1.session_id;
        let sid2 = s2.session_id;
        m.sessions.write().insert(sid1, s1);
        m.sessions.write().insert(sid2, s2);

        let removed = m.remove_session(sid1);
        assert!(removed.is_some());

        assert_eq!(m.session_count(), 1);
        assert!(m.current(sid1).is_none());
        assert!(m.current(sid2).is_some());
    }

    #[test]
    fn deactivate_all_clears_map() {
        let m = SessionManager::new();
        for _ in 0..3 {
            let s = fake_session(Uuid::new_v4());
            m.sessions.write().insert(s.session_id, s);
        }
        assert_eq!(m.session_count(), 3);
        m.remove_all_sessions();
        assert_eq!(m.session_count(), 0);
    }
}
