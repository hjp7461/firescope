//! `QueryHistoryRepository`의 Tauri 구현 — `tauri-plugin-store`
//! (`query-history.json`). `TauriProfileRepository`와 동일 패턴.

use std::sync::Arc;

use tauri::Runtime;
use tauri_plugin_store::Store;

use crate::error::{AppError, AppResult};
use crate::query::history::{QueryHistoryData, QueryHistoryRepository};

/// `query-history.json` 안에서 전체 데이터를 담는 단일 키.
const DATA_KEY: &str = "data";

pub struct TauriQueryHistoryRepository<R: Runtime> {
    store: Arc<Store<R>>,
}

impl<R: Runtime> TauriQueryHistoryRepository<R> {
    pub fn new(store: Arc<Store<R>>) -> Self {
        Self { store }
    }
}

impl<R: Runtime> QueryHistoryRepository for TauriQueryHistoryRepository<R> {
    fn load(&self) -> AppResult<QueryHistoryData> {
        match self.store.get(DATA_KEY) {
            Some(value) => serde_json::from_value(value).map_err(|_| {
                AppError::io("query-history.json is corrupt or has an incompatible schema")
            }),
            None => Ok(QueryHistoryData::default()),
        }
    }

    fn save(&self, data: &QueryHistoryData) -> AppResult<()> {
        let value = serde_json::to_value(data)
            .map_err(|_| AppError::internal("failed to serialize query history"))?;
        self.store.set(DATA_KEY, value);
        self.store
            .save()
            .map_err(|e| AppError::io(format!("failed to write query-history.json: {e}")))
    }
}
