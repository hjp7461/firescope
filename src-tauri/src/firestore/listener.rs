//! Realtime 리스너 (`docs/03-ipc-contract.md` §8.5, Phase 11).
//!
//! `FirestoreDb::create_listener` + `FirestoreMemListenStateStorage` 기반.
//! 읽기 전용 — 어떤 쓰기 메서드도 노출하지 않는다 (원칙 1).
//!
//! listener는 세션 단위로 격리되며 `ListenerRegistry`가 lifecycle을 관리한다.
//! 세션 deactivate 시 그 세션의 모든 listener가 자동 shutdown된다.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use firestore::{
    FirestoreDb, FirestoreListenEvent, FirestoreListener, FirestoreListenerTarget,
    FirestoreMemListenStateStorage,
};
use gcloud_sdk::google::firestore::v1::listen_response;
use parking_lot::RwLock;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::firestore::decode_document;
use crate::query::dsl::{ListenerDsl, QueryTarget};
use crate::query::{qualify_parent, translate, validate};

/// gRPC target_id는 i32라서 우리 listener_id(UUID) 그대로 못 씀.
/// 한 listener에는 단일 target만 등록하므로 충돌 위험 없는 고정 ID 사용.
const SINGLE_TARGET_ID: u32 = 1;

/// 활성 listener의 메타데이터 + 이벤트 카운터.
#[derive(Clone)]
pub struct ListenerInfo {
    pub listener_id: String,
    pub session_id: Uuid,
    pub target: QueryTarget,
    pub started_at: DateTime<Utc>,
    last_event_at: Arc<RwLock<Option<DateTime<Utc>>>>,
    event_count: Arc<AtomicU64>,
}

impl ListenerInfo {
    fn record_event(&self) {
        self.event_count.fetch_add(1, Ordering::Relaxed);
        *self.last_event_at.write() = Some(Utc::now());
    }

    pub fn snapshot(&self) -> ListenerInfoDto {
        ListenerInfoDto {
            listener_id: self.listener_id.clone(),
            session_id: self.session_id,
            target: self.target.clone(),
            started_at: self.started_at,
            last_event_at: *self.last_event_at.read(),
            event_count: self.event_count.load(Ordering::Relaxed),
        }
    }
}

/// IPC `ListenerInfo` 직렬화 형태 (`docs/03-ipc-contract.md` v0.10).
#[derive(Debug, Clone, Serialize)]
pub struct ListenerInfoDto {
    pub listener_id: String,
    pub session_id: Uuid,
    pub target: QueryTarget,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event_at: Option<DateTime<Utc>>,
    pub event_count: u64,
}

/// 내부 핸들 — `FirestoreListener`는 자체 task를 spawn하므로 여기서는
/// shutdown 호출용 핸들만 유지한다.
struct ListenerHandle {
    info: ListenerInfo,
    /// `firestore::FirestoreListener::shutdown()`은 `&mut self` + `await`를
    /// 요구하므로 `tokio::sync::Mutex`로 감싼다 (parking_lot guard는 await을
    /// 가로지를 수 없다).
    inner: tokio::sync::Mutex<FirestoreListener<FirestoreDb, FirestoreMemListenStateStorage>>,
}

/// listener_id 기반 활성 리스너 레지스트리. 세션 단위로 격리된다.
#[derive(Default)]
pub struct ListenerRegistry {
    inner: parking_lot::Mutex<HashMap<String, Arc<ListenerHandle>>>,
}

impl ListenerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 활성 listener 메타데이터 목록.
    pub fn list(&self) -> Vec<ListenerInfoDto> {
        self.inner
            .lock()
            .values()
            .map(|h| h.info.snapshot())
            .collect()
    }

    fn insert(&self, handle: Arc<ListenerHandle>) -> Option<Arc<ListenerHandle>> {
        self.inner
            .lock()
            .insert(handle.info.listener_id.clone(), handle)
    }

    fn take(&self, listener_id: &str) -> Option<Arc<ListenerHandle>> {
        self.inner.lock().remove(listener_id)
    }

    fn take_for_session(&self, session_id: Uuid) -> Vec<Arc<ListenerHandle>> {
        let mut guard = self.inner.lock();
        let ids: Vec<String> = guard
            .iter()
            .filter(|(_, h)| h.info.session_id == session_id)
            .map(|(id, _)| id.clone())
            .collect();
        ids.into_iter().filter_map(|id| guard.remove(&id)).collect()
    }

    fn take_all(&self) -> Vec<Arc<ListenerHandle>> {
        self.inner.lock().drain().map(|(_, v)| v).collect()
    }
}

impl ListenerRegistry {
    /// 세션 deactivate 시 호출 — 그 세션의 모든 listener를 shutdown한다.
    /// `listener:status` 이벤트는 emit하지 않는다 (정리만 수행).
    pub async fn shutdown_session(&self, session_id: Uuid) {
        for handle in self.take_for_session(session_id) {
            shutdown_one(&handle).await;
        }
    }

    /// 모든 listener를 shutdown한다 (앱 종료/`deactivate_all` 시점).
    pub async fn shutdown_all(&self) {
        for handle in self.take_all() {
            shutdown_one(&handle).await;
        }
    }
}

async fn shutdown_one(handle: &ListenerHandle) {
    let mut guard = handle.inner.lock().await;
    if let Err(err) = guard.shutdown().await {
        tracing::warn!(
            target: "listener",
            listener_id = %handle.info.listener_id,
            error = %err,
            "failed to shutdown listener"
        );
    }
}

// --- 이벤트 페이로드 ---

#[derive(Serialize, Clone)]
struct ChangePayload {
    session_id: Uuid,
    kind: ChangeKind,
    doc: crate::firestore::Document,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum ChangeKind {
    /// 문서 추가 또는 수정. 첫 스냅샷의 문서도 동일하게 보낸다 —
    /// 프론트는 path를 키로 upsert하면 되며, 첫 스냅샷 종료는
    /// `listener:status: ready` 이벤트로 구분한다.
    Modified,
    /// 삭제 또는 결과집합에서 제외(DocumentDelete/DocumentRemove).
    Removed,
}

#[derive(Serialize, Clone)]
struct StatusPayload {
    session_id: Uuid,
    status: StatusKind,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum StatusKind {
    Initial,
    Ready,
    Reset,
}

/// `start_listener` 커맨드의 본체.
///
/// 동일 `listener_id`로 다시 호출되면 기존 listener를 shutdown한 뒤 새로
/// 시작한다 (DSL 수정 후 재시작 시나리오).
pub async fn start_listener<R: Runtime>(
    app: AppHandle<R>,
    db: FirestoreDb,
    registry: Arc<ListenerRegistry>,
    listener_id: String,
    session_id: Uuid,
    dsl: ListenerDsl,
) -> AppResult<()> {
    // 1) 검증 — query::validate를 재사용해 일관된 제약 적용 (where 절 한정).
    let query_dsl = dsl.to_query_dsl();
    validate(&query_dsl)?;

    // 2) 기존 listener가 있으면 shutdown.
    if let Some(existing) = registry.take(&listener_id) {
        shutdown_one(&existing).await;
    }

    // 3) translate로 FirestoreQueryParams 생성 → fluent listen → add_target.
    let mut params = translate(&query_dsl)?;
    qualify_parent(&mut params, db.get_documents_path());
    let mut listener = db
        .create_listener(FirestoreMemListenStateStorage::new())
        .await
        .map_err(|_| AppError::Firestore {
            message: "failed to create realtime listener".into(),
        })?;

    // fluent 빌더는 listen 시점에 params를 다시 짜는데, 우리는 이미
    // `query::translate`로 변환된 동일 `FirestoreQueryParams`를 가지고 있으므로
    // `FirestoreListenerTargetParams`에 직접 주입한다 (translate 재사용).
    use firestore::{FirestoreListenerTargetParams, FirestoreTargetType};
    let target_params = FirestoreListenerTargetParams::new(
        FirestoreListenerTarget::new(SINGLE_TARGET_ID),
        FirestoreTargetType::Query(params),
        std::collections::HashMap::new(),
    );
    listener
        .add_target(target_params)
        .map_err(|_| AppError::Firestore {
            message: "failed to register listener target".into(),
        })?;

    // 4) 메타데이터 준비 + 이벤트 콜백 등록.
    let info = ListenerInfo {
        listener_id: listener_id.clone(),
        session_id,
        target: dsl.target.clone(),
        started_at: Utc::now(),
        last_event_at: Arc::new(RwLock::new(None)),
        event_count: Arc::new(AtomicU64::new(0)),
    };

    let change_ev = format!("listener:change:{listener_id}");
    let status_ev = format!("listener:status:{listener_id}");

    // 첫 스냅샷 완료 추적 — TargetChange::CURRENT가 도착하기 전의 added는 초기 스냅샷.
    let app_for_cb = app.clone();
    let info_for_cb = info.clone();
    listener
        .start(move |event| {
            let app = app_for_cb.clone();
            let info = info_for_cb.clone();
            let change_ev = change_ev.clone();
            let status_ev = status_ev.clone();
            async move {
                dispatch_event(&app, &info, &change_ev, &status_ev, event);
                Ok(())
            }
        })
        .await
        .map_err(|_| AppError::Firestore {
            message: "failed to start realtime listener".into(),
        })?;

    // 5) 초기 status emit.
    let _ = app.emit(
        &format!("listener:status:{listener_id}"),
        StatusPayload {
            session_id,
            status: StatusKind::Initial,
        },
    );

    tracing::info!(
        target: "listener",
        listener_id = %listener_id,
        session_id = %session_id,
        "realtime listener started"
    );

    // 6) 레지스트리에 보관.
    let handle = Arc::new(ListenerHandle {
        info,
        inner: tokio::sync::Mutex::new(listener),
    });
    registry.insert(handle);

    Ok(())
}

/// `stop_listener` 커맨드 본체. 알 수 없는 id는 idempotent.
pub async fn stop_listener(registry: Arc<ListenerRegistry>, listener_id: &str) -> AppResult<()> {
    if let Some(handle) = registry.take(listener_id) {
        shutdown_one(&handle).await;
        tracing::info!(
            target: "listener",
            listener_id = %listener_id,
            "realtime listener stopped"
        );
    }
    Ok(())
}

/// 단일 listen 이벤트를 IPC 이벤트로 변환.
fn dispatch_event<R: Runtime>(
    app: &AppHandle<R>,
    info: &ListenerInfo,
    change_ev: &str,
    status_ev: &str,
    event: FirestoreListenEvent,
) {
    use listen_response::ResponseType;
    match event {
        ResponseType::DocumentChange(change) => {
            if let Some(doc_proto) = change.document.as_ref() {
                let doc = decode_document(doc_proto);
                // removed_target_ids가 비어있지 않으면 우리 target에서 제외된
                // 것 — 사용자 관점에선 "removed". 그 외는 added/modified를
                // 구분하지 않고 일괄 modified로 보낸다 (첫 스냅샷에서도 동일).
                // 프론트는 path를 키로 기존 결과집합에 upsert만 하면 된다.
                info.record_event();
                let _ = app.emit(
                    change_ev,
                    ChangePayload {
                        session_id: info.session_id,
                        kind: if change.removed_target_ids.is_empty() {
                            ChangeKind::Modified
                        } else {
                            ChangeKind::Removed
                        },
                        doc,
                    },
                );
            }
        }
        ResponseType::DocumentDelete(del) => {
            info.record_event();
            let doc = crate::firestore::Document {
                path: strip_db_prefix(&del.document).to_string(),
                id: last_segment(&del.document).to_string(),
                parent: parent_path(&del.document).to_string(),
                data: std::collections::BTreeMap::new(),
                create_time: None,
                update_time: None,
            };
            let _ = app.emit(
                change_ev,
                ChangePayload {
                    session_id: info.session_id,
                    kind: ChangeKind::Removed,
                    doc,
                },
            );
        }
        ResponseType::DocumentRemove(rem) => {
            info.record_event();
            let doc = crate::firestore::Document {
                path: strip_db_prefix(&rem.document).to_string(),
                id: last_segment(&rem.document).to_string(),
                parent: parent_path(&rem.document).to_string(),
                data: std::collections::BTreeMap::new(),
                create_time: None,
                update_time: None,
            };
            let _ = app.emit(
                change_ev,
                ChangePayload {
                    session_id: info.session_id,
                    kind: ChangeKind::Removed,
                    doc,
                },
            );
        }
        ResponseType::TargetChange(tc) => {
            use gcloud_sdk::google::firestore::v1::target_change::TargetChangeType;
            match TargetChangeType::try_from(tc.target_change_type).ok() {
                Some(TargetChangeType::Current) => {
                    let _ = app.emit(
                        status_ev,
                        StatusPayload {
                            session_id: info.session_id,
                            status: StatusKind::Ready,
                        },
                    );
                }
                Some(TargetChangeType::Reset) => {
                    let _ = app.emit(
                        status_ev,
                        StatusPayload {
                            session_id: info.session_id,
                            status: StatusKind::Reset,
                        },
                    );
                }
                _ => {}
            }
        }
        ResponseType::Filter(_) => {
            // 간헐적 일관성 알림 — 현재는 무시 (필요해지면 별도 status로 emit).
        }
    }
}

/// `projects/p/databases/d/documents/users/abc` → `users/abc`.
fn strip_db_prefix(full: &str) -> &str {
    full.rfind("/documents/")
        .map(|i| &full[i + "/documents/".len()..])
        .unwrap_or(full)
}

fn last_segment(full: &str) -> &str {
    full.rsplit('/').next().unwrap_or("")
}

fn parent_path(full: &str) -> &str {
    let path = strip_db_prefix(full);
    match path.rfind('/') {
        Some(i) => &path[..i],
        None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_db_prefix_extracts_user_path() {
        assert_eq!(
            strip_db_prefix("projects/demo/databases/(default)/documents/users/abc"),
            "users/abc"
        );
        // 접두사가 없으면 원본 그대로.
        assert_eq!(strip_db_prefix("users/abc"), "users/abc");
    }

    #[test]
    fn last_segment_returns_doc_id() {
        assert_eq!(last_segment("users/abc"), "abc");
        assert_eq!(last_segment("a/b/c/d"), "d");
        assert_eq!(last_segment(""), "");
    }

    #[test]
    fn parent_path_strips_db_prefix_and_id() {
        assert_eq!(
            parent_path("projects/demo/databases/(default)/documents/users/abc"),
            "users"
        );
        assert_eq!(parent_path("orgs/o1/teams/t1"), "orgs/o1/teams");
        assert_eq!(parent_path("root"), "");
    }
}
