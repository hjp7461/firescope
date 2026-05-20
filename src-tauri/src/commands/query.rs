//! 컬렉션/문서/쿼리/히스토리 커맨드 (`docs/03-ipc-contract.md` §3·§4·§8).
//!
//! 조회 계열은 활성 세션의 라이브 `FirestoreClient`를 사용한다. 세션이
//! 없으면 `SessionManager::firestore()`가 `no_session` 에러를 반환한다.
//! 히스토리 계열은 세션과 무관하게 프로파일별 로컬 데이터를 다룬다.

use std::path::PathBuf;
use std::sync::Arc;

use firestore::{
    FirestoreGetByIdSupport, FirestoreListCollectionIdsParams, FirestoreListingSupport,
    FirestoreQuerySupport,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::firestore::streaming::run_query;
use crate::firestore::{decode_document, Document, ExportFormat, ExportSource};
use crate::query::dsl::QueryDsl;
use crate::query::history::HistoryEntry;
use crate::query::stats::{self, StatsReport};
use crate::query::{post_filter, translate, validate};
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
    // 새 쿼리가 시작되면 이전 stream들의 sink를 정리한다 (원칙 5 — 운영 데이터를
    // 디스크에 오래 두지 않음). 사용자가 export하지 않으면 자동 폐기.
    registry.cancel_all();
    // TODO(multi-tab Task 8): pass real session_id from the command param
    let (_flag, sink) = registry.register(&stream_id, uuid::Uuid::nil());
    // 즉시 반환, 결과는 이벤트로 (원칙 6).
    tokio::spawn(run_query(app, client, registry, sink, stream_id, dsl));
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn cancel_stream(state: State<'_, AppState>, stream_id: String) -> AppResult<()> {
    let streams = state.sessions.streams();
    streams.cancel(&stream_id);
    // 사용자가 명시적으로 취소한 결과는 export 대상이 아니므로 즉시 폐기한다.
    streams.drop_stream(&stream_id);
    Ok(())
}

// --- Phase 6: Export / Count (`docs/03-ipc-contract.md` §4·§5 v0.5) ---

#[derive(Deserialize)]
pub struct ExportResultParams {
    pub stream_id: String,
    pub format: ExportFormat,
    pub path: PathBuf,
    #[serde(default)]
    pub source: Option<ExportSource>,
}

#[derive(Serialize)]
pub struct ExportResultResponse {
    pub written_bytes: u64,
    pub path: PathBuf,
    pub row_count: usize,
}

/// 활성 스트림의 디스크 sink에서 결과를 읽어 지정 포맷으로 파일에 쓴다.
/// sink가 없으면(=다른 쿼리가 시작되었거나 세션이 해제됨) `internal` 에러.
#[tauri::command(rename_all = "snake_case")]
pub fn export_result(
    state: State<'_, AppState>,
    params: ExportResultParams,
) -> AppResult<ExportResultResponse> {
    let streams = state.sessions.streams();
    let sink = streams.sink(&params.stream_id).ok_or_else(|| {
        AppError::Internal {
            message: format!("no result sink for stream {}", params.stream_id),
        }
    })?;
    let source = params.source.unwrap_or_default();
    let stats = {
        let guard = sink.lock();
        match params.format {
            ExportFormat::Json => guard.write_json(&params.path, source),
            ExportFormat::Ndjson => guard.write_ndjson(&params.path, source),
            ExportFormat::Csv => guard.write_csv(&params.path, source),
        }
        .map_err(|e| AppError::Io {
            message: format!("failed to write export file: {e}"),
        })?
    };
    tracing::info!(
        target: "query",
        stream_id = %params.stream_id,
        format = ?params.format,
        source = ?source,
        row_count = stats.row_count,
        written_bytes = stats.written_bytes,
        op = "export_result",
        "exported query result"
    );
    Ok(ExportResultResponse {
        written_bytes: stats.written_bytes,
        path: params.path,
        row_count: stats.row_count,
    })
}

#[derive(Serialize)]
pub struct QueryCountResponse {
    pub matched: u64,
    pub scanned: u64,
}

/// DSL을 실행해 post_filter 통과 건수와 스캔 건수를 반환한다.
/// Firestore aggregation API는 백로그 — 현재는 스트리밍 카운트.
#[tauri::command(rename_all = "snake_case")]
pub async fn query_count(
    state: State<'_, AppState>,
    dsl: QueryDsl,
) -> AppResult<QueryCountResponse> {
    let client = state.sessions.firestore()?;
    validate(&dsl)?;
    let params = translate(&dsl)?;
    let matcher = dsl
        .post_filter
        .as_ref()
        .map(post_filter::compile)
        .transpose()?;

    let mut stream = client
        .db
        .stream_query_doc_with_errors(params)
        .await
        .map_err(|_| AppError::Firestore {
            message: "failed to start count query stream".into(),
        })?;

    let mut matched: u64 = 0;
    let mut scanned: u64 = 0;
    while let Some(item) = stream.next().await {
        let item = item.map_err(|_| AppError::Firestore {
            message: "error while counting query results".into(),
        })?;
        scanned += 1;
        if let Some(m) = matcher.as_ref() {
            let doc = decode_document(&item);
            if m.matches(&doc.data) {
                matched += 1;
            }
        } else {
            matched += 1;
        }
    }
    Ok(QueryCountResponse { matched, scanned })
}

// --- Phase 9: 컬렉션 통계 (`docs/03-ipc-contract.md` §5 compute_stats) ---

#[derive(Deserialize)]
pub struct ComputeStatsParams {
    pub stream_id: String,
    #[serde(default)]
    pub source: Option<ExportSource>,
    #[serde(default)]
    pub top_samples: Option<usize>,
}

const DEFAULT_TOP_SAMPLES: usize = 5;

/// 활성 스트림의 디스크 sink에서 결과를 읽어 필드별 통계를 산출한다.
/// sink가 없으면(=다른 쿼리가 시작되었거나 세션이 해제됨) `internal` 에러.
#[tauri::command(rename_all = "snake_case")]
pub fn compute_stats(
    state: State<'_, AppState>,
    params: ComputeStatsParams,
) -> AppResult<StatsReport> {
    let streams = state.sessions.streams();
    let sink = streams
        .sink(&params.stream_id)
        .ok_or_else(|| AppError::Internal {
            message: format!("no result sink for stream {}", params.stream_id),
        })?;
    let source = params.source.unwrap_or_default();
    let top_samples = stats::clamp_top_samples(params.top_samples.unwrap_or(DEFAULT_TOP_SAMPLES));

    let docs: Vec<Document> = {
        let guard = sink.lock();
        let iter = guard.iter(source).map_err(|e| AppError::Io {
            message: format!("failed to read result sink: {e}"),
        })?;
        let mut out = Vec::new();
        for item in iter {
            out.push(item.map_err(|e| AppError::Io {
                message: format!("failed to read result sink: {e}"),
            })?);
        }
        out
    };

    let source_label = match source {
        ExportSource::Matched => "matched",
        ExportSource::Scanned => "scanned",
    };
    let report = stats::compute_field_stats(docs, source_label, top_samples);

    tracing::info!(
        target: "query",
        stream_id = %params.stream_id,
        source = ?source,
        sample_size = report.sample_size,
        field_count = report.fields.len(),
        op = "compute_stats",
        "computed collection stats"
    );

    Ok(report)
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

#[tauri::command(rename_all = "snake_case")]
pub fn pin_query_history(
    state: State<'_, AppState>,
    profile_id: Uuid,
    entry_id: Uuid,
    pinned: bool,
) -> AppResult<HistoryEntry> {
    state.history.pin(profile_id, entry_id, pinned)
}
