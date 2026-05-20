//! 탭 그룹 영속화 IPC. `tauri-plugin-store` 패스스루.
//!
//! 도메인 로직 없음 (원칙 4) — 프론트 `tabsStore`가 권위.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;
use uuid::Uuid;

use crate::error::AppResult;

const STORE_FILE: &str = "tabs.json";
const KEY: &str = "bundle";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct TabRecord {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<Uuid>,
    pub order: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct TabBundle {
    pub version: u32,
    pub tabs: Vec<TabRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_tab_id: Option<Uuid>,
}

impl Default for TabBundle {
    fn default() -> Self {
        Self {
            version: 1,
            tabs: vec![],
            active_tab_id: None,
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn list_tabs<R: Runtime>(app: AppHandle<R>) -> AppResult<TabBundle> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| crate::error::AppError::io(format!("open tabs store: {e}")))?;
    let raw = store.get(KEY).unwrap_or(serde_json::Value::Null);
    if raw.is_null() {
        return Ok(TabBundle::default());
    }
    serde_json::from_value(raw)
        .map_err(|e| crate::error::AppError::internal(format!("tabs.json malformed: {e}")))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn save_tabs<R: Runtime>(
    app: AppHandle<R>,
    bundle: TabBundle,
) -> AppResult<()> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| crate::error::AppError::io(format!("open tabs store: {e}")))?;
    store.set(
        KEY,
        serde_json::to_value(&bundle)
            .map_err(|e| crate::error::AppError::internal(format!("serialize: {e}")))?,
    );
    store
        .save()
        .map_err(|e| crate::error::AppError::io(format!("save tabs store: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bundle_is_v1_empty() {
        let b = TabBundle::default();
        assert_eq!(b.version, 1);
        assert!(b.tabs.is_empty());
        assert!(b.active_tab_id.is_none());
    }

    #[test]
    fn roundtrip_serde() {
        let original = TabBundle {
            version: 1,
            tabs: vec![
                TabRecord { id: Uuid::nil(), profile_id: None, order: 0 },
                TabRecord {
                    id: Uuid::new_v4(),
                    profile_id: Some(Uuid::new_v4()),
                    order: 1,
                },
            ],
            active_tab_id: Some(Uuid::new_v4()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: TabBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }
}
