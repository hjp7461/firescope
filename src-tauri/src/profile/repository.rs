//! 프로파일 메타데이터 영속화 포트.
//!
//! 원칙 4: 도메인은 `tauri-plugin-store`를 직접 모른다. 실제 어댑터
//! (`adapters::TauriProfileRepository`)가 이 trait를 구현하고, 테스트는
//! [`InMemoryProfileRepository`]를 주입한다.

use crate::error::AppResult;
use crate::profile::model::ProfileStoreData;

/// `profiles.json` 영속화 포트. `ProfileStoreData` 단위로만 주고받는다.
pub trait ProfileRepository: Send + Sync {
    fn load(&self) -> AppResult<ProfileStoreData>;
    fn save(&self, data: &ProfileStoreData) -> AppResult<()>;
}

/// 테스트용 인메모리 저장소.
#[cfg(test)]
pub struct InMemoryProfileRepository {
    inner: parking_lot::Mutex<ProfileStoreData>,
}

#[cfg(test)]
impl InMemoryProfileRepository {
    pub fn new() -> Self {
        Self {
            inner: parking_lot::Mutex::new(ProfileStoreData::default()),
        }
    }
}

#[cfg(test)]
impl ProfileRepository for InMemoryProfileRepository {
    fn load(&self) -> AppResult<ProfileStoreData> {
        Ok(self.inner.lock().clone())
    }
    fn save(&self, data: &ProfileStoreData) -> AppResult<()> {
        *self.inner.lock() = data.clone();
        Ok(())
    }
}
