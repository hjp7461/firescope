//! 쿼리 히스토리 — 프로파일별 격리 영속화 (순수 도메인).
//!
//! `docs/03-ipc-contract.md` §8, `docs/07-profiles.md` 저장 모델.
//! 원칙 4: `tauri::*`/`tauri-plugin-store`를 직접 모른다. 실제 어댑터
//! (`adapters::TauriQueryHistoryRepository`)가 [`QueryHistoryRepository`]를
//! 구현하고, 테스트는 [`InMemoryQueryHistoryRepository`]를 주입한다.

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppResult;
use crate::query::dsl::QueryDsl;

/// 프로파일당 보관 상한. 초과 시 가장 오래된 항목부터 제거.
const MAX_PER_PROFILE: usize = 100;

/// 히스토리 1건 (`docs/03-ipc-contract.md` `QueryHistoryEntry`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: Uuid,
    pub dsl: QueryDsl,
    pub executed_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub took_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_count: Option<u64>,
}

/// `query-history.json` 전체 — 프로파일 ID → 최신순 항목 목록.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryHistoryData {
    by_profile: BTreeMap<Uuid, Vec<HistoryEntry>>,
}

/// 영속화 포트. `QueryHistoryData` 단위로만 주고받는다.
pub trait QueryHistoryRepository: Send + Sync {
    fn load(&self) -> AppResult<QueryHistoryData>;
    fn save(&self, data: &QueryHistoryData) -> AppResult<()>;
}

/// 두 DSL이 의미상 동일한가 (serde 직렬화 형태로 비교 — DSL 트리 전체에
/// `PartialEq`를 강제하지 않기 위함).
fn same_dsl(a: &QueryDsl, b: &QueryDsl) -> bool {
    match (serde_json::to_value(a), serde_json::to_value(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

pub struct QueryHistoryManager {
    repo: Arc<dyn QueryHistoryRepository>,
    data: RwLock<QueryHistoryData>,
}

impl QueryHistoryManager {
    pub fn new(repo: Arc<dyn QueryHistoryRepository>) -> AppResult<Self> {
        let data = repo.load()?;
        Ok(Self {
            repo,
            data: RwLock::new(data),
        })
    }

    /// 락을 짧게 잡고 변경 → 스냅샷 → 락 밖에서 영속화 (원칙 8).
    fn persist(&self) -> AppResult<()> {
        let snapshot = self.data.read().clone();
        self.repo.save(&snapshot)
    }

    /// 프로파일의 히스토리(최신순 복제본).
    pub fn list(&self, profile_id: Uuid) -> Vec<HistoryEntry> {
        self.data
            .read()
            .by_profile
            .get(&profile_id)
            .cloned()
            .unwrap_or_default()
    }

    /// 실행 직후 기록. 최신 항목과 DSL이 같으면 새 행을 추가하지 않고
    /// 그 항목을 갱신(timestamp/metric)하며 맨 위로 올린다. 상한 초과 시
    /// 가장 오래된 항목 제거.
    pub fn add(
        &self,
        profile_id: Uuid,
        dsl: QueryDsl,
        took_ms: Option<u64>,
        result_count: Option<u64>,
    ) -> AppResult<HistoryEntry> {
        let entry = {
            let mut data = self.data.write();
            let list = data.by_profile.entry(profile_id).or_default();

            match list.first_mut() {
                Some(top) if same_dsl(&top.dsl, &dsl) => {
                    top.executed_at = Utc::now();
                    top.took_ms = took_ms;
                    top.result_count = result_count;
                    top.clone()
                }
                _ => {
                    let entry = HistoryEntry {
                        id: Uuid::new_v4(),
                        dsl,
                        executed_at: Utc::now(),
                        took_ms,
                        result_count,
                    };
                    list.insert(0, entry.clone());
                    list.truncate(MAX_PER_PROFILE);
                    entry
                }
            }
        };
        self.persist()?;
        Ok(entry)
    }

    /// 단일 항목 제거. 없는 id는 조용히 무시(idempotent).
    pub fn remove(&self, profile_id: Uuid, entry_id: Uuid) -> AppResult<()> {
        {
            let mut data = self.data.write();
            if let Some(list) = data.by_profile.get_mut(&profile_id) {
                list.retain(|e| e.id != entry_id);
            }
        }
        self.persist()
    }

    /// 프로파일의 히스토리 전체 삭제. (프로파일 삭제 캐스케이드에도 사용)
    pub fn clear(&self, profile_id: Uuid) -> AppResult<()> {
        {
            self.data.write().by_profile.remove(&profile_id);
        }
        self.persist()
    }
}

#[cfg(test)]
pub struct InMemoryQueryHistoryRepository {
    inner: parking_lot::Mutex<QueryHistoryData>,
}

#[cfg(test)]
impl InMemoryQueryHistoryRepository {
    pub fn new() -> Self {
        Self {
            inner: parking_lot::Mutex::new(QueryHistoryData::default()),
        }
    }
}

#[cfg(test)]
impl QueryHistoryRepository for InMemoryQueryHistoryRepository {
    fn load(&self) -> AppResult<QueryHistoryData> {
        Ok(self.inner.lock().clone())
    }
    fn save(&self, data: &QueryHistoryData) -> AppResult<()> {
        *self.inner.lock() = data.clone();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::dsl::{QueryDsl, QueryTarget};

    fn mgr() -> (QueryHistoryManager, Arc<InMemoryQueryHistoryRepository>) {
        let repo = Arc::new(InMemoryQueryHistoryRepository::new());
        let m = QueryHistoryManager::new(
            repo.clone() as Arc<dyn QueryHistoryRepository>
        )
        .unwrap();
        (m, repo)
    }

    fn dsl(path: &str) -> QueryDsl {
        QueryDsl {
            target: QueryTarget::Collection { path: path.into() },
            r#where: vec![],
            order_by: vec![],
            limit: None,
            start_after: None,
            end_before: None,
            select: vec![],
            post_filter: None,
        }
    }

    #[test]
    fn add_then_list_is_newest_first() {
        let (m, _) = mgr();
        let p = Uuid::new_v4();
        m.add(p, dsl("a"), None, None).unwrap();
        m.add(p, dsl("b"), None, None).unwrap();
        let list = m.list(p);
        assert_eq!(list.len(), 2);
        assert!(matches!(
            &list[0].dsl.target,
            QueryTarget::Collection { path } if path == "b"
        ));
    }

    #[test]
    fn consecutive_identical_dsl_is_deduped_and_updated() {
        let (m, _) = mgr();
        let p = Uuid::new_v4();
        let first = m.add(p, dsl("a"), Some(10), Some(1)).unwrap();
        let second = m.add(p, dsl("a"), Some(20), Some(2)).unwrap();
        let list = m.list(p);
        assert_eq!(list.len(), 1, "identical DSL must not create a new row");
        assert_eq!(first.id, second.id, "dedupe keeps the same entry id");
        assert_eq!(list[0].took_ms, Some(20));
        assert_eq!(list[0].result_count, Some(2));
    }

    #[test]
    fn non_consecutive_identical_dsl_is_not_deduped() {
        let (m, _) = mgr();
        let p = Uuid::new_v4();
        m.add(p, dsl("a"), None, None).unwrap();
        m.add(p, dsl("b"), None, None).unwrap();
        m.add(p, dsl("a"), None, None).unwrap();
        assert_eq!(m.list(p).len(), 3);
    }

    #[test]
    fn caps_at_100_dropping_oldest() {
        let (m, _) = mgr();
        let p = Uuid::new_v4();
        for i in 0..105 {
            m.add(p, dsl(&format!("c{i}")), None, None).unwrap();
        }
        let list = m.list(p);
        assert_eq!(list.len(), 100);
        assert!(matches!(
            &list[0].dsl.target,
            QueryTarget::Collection { path } if path == "c104"
        ));
        assert!(matches!(
            &list[99].dsl.target,
            QueryTarget::Collection { path } if path == "c5"
        ));
    }

    #[test]
    fn profiles_are_isolated() {
        let (m, _) = mgr();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        m.add(p1, dsl("a"), None, None).unwrap();
        assert_eq!(m.list(p1).len(), 1);
        assert_eq!(m.list(p2).len(), 0);
    }

    #[test]
    fn remove_deletes_entry() {
        let (m, _) = mgr();
        let p = Uuid::new_v4();
        let e = m.add(p, dsl("a"), None, None).unwrap();
        m.add(p, dsl("b"), None, None).unwrap();
        m.remove(p, e.id).unwrap();
        let list = m.list(p);
        assert_eq!(list.len(), 1);
        assert!(matches!(
            &list[0].dsl.target,
            QueryTarget::Collection { path } if path == "b"
        ));
    }

    #[test]
    fn clear_empties_profile() {
        let (m, _) = mgr();
        let p = Uuid::new_v4();
        m.add(p, dsl("a"), None, None).unwrap();
        m.clear(p).unwrap();
        assert_eq!(m.list(p).len(), 0);
    }

    #[test]
    fn changes_are_persisted_through_repo() {
        let repo = Arc::new(InMemoryQueryHistoryRepository::new());
        let p = Uuid::new_v4();
        {
            let m = QueryHistoryManager::new(
                repo.clone() as Arc<dyn QueryHistoryRepository>
            )
            .unwrap();
            m.add(p, dsl("a"), None, None).unwrap();
        }
        // 새 매니저가 동일 repo에서 복구.
        let m2 =
            QueryHistoryManager::new(repo as Arc<dyn QueryHistoryRepository>).unwrap();
        assert_eq!(m2.list(p).len(), 1);
    }
}
