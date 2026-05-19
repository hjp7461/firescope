//! 컬렉션/문서/쿼리/히스토리 커맨드 (`docs/03-ipc-contract.md` §3·§4·§8).
//!
//! 조회 계열은 활성 세션의 라이브 `FirestoreClient`를 사용한다. 세션이
//! 없으면 `SessionManager::firestore()`가 `no_session` 에러를 반환한다.
//! 히스토리 계열은 세션과 무관하게 프로파일별 로컬 데이터를 다룬다.

use std::sync::Arc;

use firestore::{
    FirestoreGetByIdSupport, FirestoreListCollectionIdsParams, FirestoreListingSupport,
};
use serde::Deserialize;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::firestore::streaming::run_query;
use crate::firestore::{decode_document, Document};
use crate::query::dsl::QueryDsl;
use crate::query::history::HistoryEntry;
use crate::state::AppState;

#[tauri::command(rename_all = "snake_case")]
pub async fn list_collections(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let client = state.sessions.firestore()?;
    let res = client
        .db
        .list_collection_ids(FirestoreListCollectionIdsParams::new())
        .await
        .map_err(|_| AppError::Firestore {
            message: "failed to list root collections".into(),
        })?;
    Ok(res.collection_ids)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn list_subcollections(
    state: State<'_, AppState>,
    document_path: String,
) -> AppResult<Vec<String>> {
    let client = state.sessions.firestore()?;
    let parent = format!("{}/{document_path}", client.db.get_documents_path());
    let mut params = FirestoreListCollectionIdsParams::new();
    params.parent = Some(parent);
    let res = client
        .db
        .list_collection_ids(params)
        .await
        .map_err(|_| AppError::Firestore {
            message: "failed to list subcollections".into(),
        })?;
    Ok(res.collection_ids)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_document(state: State<'_, AppState>, path: String) -> AppResult<Option<Document>> {
    let client = state.sessions.firestore()?;
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() || segments.len() % 2 != 0 {
        return Err(AppError::InvalidQuery {
            message: "document path must have an even number of segments".into(),
        });
    }
    let id = segments[segments.len() - 1];
    let collection = segments[segments.len() - 2];
    let parent_segments = &segments[..segments.len() - 2];

    let result = if parent_segments.is_empty() {
        client.db.get_doc(collection, id, None).await
    } else {
        let parent = format!(
            "{}/{}",
            client.db.get_documents_path(),
            parent_segments.join("/")
        );
        client.db.get_doc_at(&parent, collection, id, None).await
    };

    match result {
        Ok(doc) => Ok(Some(decode_document(&doc))),
        // 문서 부재는 null로 (다른 에러는 마스킹하지 않고 전파).
        Err(e) if format!("{e:?}").contains("DataNotFound") => Ok(None),
        Err(_) => Err(AppError::Firestore {
            message: "failed to fetch document".into(),
        }),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn query_documents(
    app: AppHandle,
    state: State<'_, AppState>,
    stream_id: String,
    dsl: QueryDsl,
) -> AppResult<()> {
    let client = state.sessions.firestore()?;
    let registry = Arc::clone(state.sessions.streams());
    registry.register(&stream_id);
    // 즉시 반환, 결과는 이벤트로 (원칙 6).
    tokio::spawn(run_query(app, client, registry, stream_id, dsl));
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn cancel_stream(state: State<'_, AppState>, stream_id: String) -> AppResult<()> {
    state.sessions.streams().cancel(&stream_id);
    Ok(())
}

// --- 쿼리 히스토리 (`docs/03-ipc-contract.md` §8) ---
// 세션과 무관한 프로파일별 로컬 데이터. 자격증명/결과는 보관하지 않는다.

#[tauri::command(rename_all = "snake_case")]
pub fn list_query_history(
    state: State<'_, AppState>,
    profile_id: Uuid,
) -> AppResult<Vec<HistoryEntry>> {
    Ok(state.history.list(profile_id))
}

#[derive(Deserialize)]
pub struct AddQueryHistoryParams {
    pub profile_id: Uuid,
    pub dsl: QueryDsl,
    #[serde(default)]
    pub took_ms: Option<u64>,
    #[serde(default)]
    pub result_count: Option<u64>,
}

#[tauri::command(rename_all = "snake_case")]
pub fn add_query_history(
    state: State<'_, AppState>,
    params: AddQueryHistoryParams,
) -> AppResult<HistoryEntry> {
    state.history.add(
        params.profile_id,
        params.dsl,
        params.took_ms,
        params.result_count,
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn remove_query_history(
    state: State<'_, AppState>,
    profile_id: Uuid,
    entry_id: Uuid,
) -> AppResult<()> {
    state.history.remove(profile_id, entry_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn clear_query_history(
    state: State<'_, AppState>,
    profile_id: Uuid,
) -> AppResult<()> {
    state.history.clear(profile_id)
}
