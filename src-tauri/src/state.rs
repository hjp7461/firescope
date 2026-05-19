//! Tauri 관리 상태 (`app.manage`).
//!
//! Phase 1-A 범위에서는 [`ProfileManager`]만 완전 구현된다.
//! [`SessionManager`]/[`StreamRegistry`]는 Phase 1-C / Phase 2에서
//! 채워질 골격이다 — 지금은 타입과 보관 위치만 확정한다.

use parking_lot::RwLock;
use tauri::Runtime;

use crate::profile::store::ProfileManager;

/// 진행 중인 쿼리 스트림 추적기.
///
/// Phase 2에서 `stream_id → CancellationToken` 맵으로 채워진다.
/// 프로파일 전환 시 모든 스트림을 일괄 취소하는 책임을 갖는다.
#[derive(Default)]
pub struct StreamRegistry {
    _private: (),
}

impl StreamRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

/// 활성 세션 1개를 보관. Phase 1-C에서 activate/deactivate가 구현된다.
///
/// `ActiveSession`(FirestoreClient + AuthHandle)은 인증/연결 계층
/// (Phase 1-B/1-C)이 도입된 뒤 정의되므로, 지금은 빈 슬롯만 둔다.
#[derive(Default)]
pub struct SessionManager {
    active: RwLock<Option<()>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 활성 세션 존재 여부 (Phase 1-C 전까지는 항상 false).
    pub fn is_active(&self) -> bool {
        self.active.read().is_some()
    }
}

/// 앱 전역 상태. `tauri::State<AppState<R>>`로 커맨드에서 접근한다.
pub struct AppState<R: Runtime> {
    pub profiles: ProfileManager<R>,
    pub sessions: SessionManager,
    pub streams: StreamRegistry,
}

impl<R: Runtime> AppState<R> {
    pub fn new(profiles: ProfileManager<R>) -> Self {
        Self {
            profiles,
            sessions: SessionManager::new(),
            streams: StreamRegistry::new(),
        }
    }
}
