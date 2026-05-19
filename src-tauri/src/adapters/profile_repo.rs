//! `ProfileRepository`의 Tauri 구현 — `tauri-plugin-store`(`profiles.json`).

use std::sync::Arc;

use tauri::Runtime;
use tauri_plugin_store::Store;

use crate::error::{AppError, AppResult};
use crate::profile::model::ProfileStoreData;
use crate::profile::repository::ProfileRepository;

/// `profiles.json` 안에서 전체 데이터를 담는 단일 키.
const DATA_KEY: &str = "data";

pub struct TauriProfileRepository<R: Runtime> {
    store: Arc<Store<R>>,
}

impl<R: Runtime> TauriProfileRepository<R> {
    pub fn new(store: Arc<Store<R>>) -> Self {
        Self { store }
    }
}

impl<R: Runtime> ProfileRepository for TauriProfileRepository<R> {
    fn load(&self) -> AppResult<ProfileStoreData> {
        match self.store.get(DATA_KEY) {
            Some(value) => serde_json::from_value(value).map_err(|_| {
                AppError::io("profiles.json is corrupt or has an incompatible schema")
            }),
            None => Ok(ProfileStoreData::default()),
        }
    }

    fn save(&self, data: &ProfileStoreData) -> AppResult<()> {
        let value = serde_json::to_value(data)
            .map_err(|_| AppError::internal("failed to serialize profile store"))?;
        self.store.set(DATA_KEY, value);
        self.store
            .save()
            .map_err(|e| AppError::io(format!("failed to write profiles.json: {e}")))
    }
}
