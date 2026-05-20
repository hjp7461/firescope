//! 쿼리 스트리밍 — 페이지 단위로 이벤트 전송 (원칙 6: 스트리밍 우선).
//!
//! `query_documents`는 즉시 반환하고, 결과는
//! `query:chunk:<sid>` / `query:done:<sid>` / `query:error:<sid>`
//! 이벤트로 전달한다 (`docs/03-ipc-contract.md` §4).

use std::sync::Arc;
use std::time::Instant;

use firestore::FirestoreQuerySupport;
use futures::StreamExt;
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{Emitter, Runtime};

use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::firestore::{
    decode_document, extract_firestore_index_url, Document, FirestoreClient, ResultSink,
};
use crate::query::dsl::QueryDsl;
use crate::query::{post_filter, translate, validate};
use crate::state::StreamRegistry;

const PAGE: usize = 100;

#[derive(Serialize, Clone)]
struct ChunkPayload {
    session_id: Uuid,
    docs: Vec<Document>,
    page: u32,
}

#[derive(Serialize, Clone)]
struct DonePayload {
    session_id: Uuid,
    /// 후처리 통과(매칭) 문서 수 = chunk로 전달된 합계.
    total: usize,
    /// Firestore에서 가져온 전체 문서 수 (post_filter 없으면 total과 동일).
    scanned: usize,
    took_ms: u64,
    has_more: bool,
}

/// `query:error:<sid>` 페이로드. `AppError`를 `#[serde(flatten)]`으로 펼치고
/// 누락 인덱스 안내 URL을 옵션 필드로 함께 보낸다 (Phase 8-A).
/// (`docs/03-ipc-contract.md` v0.6)
#[derive(Serialize, Clone)]
struct QueryErrorPayload {
    session_id: Uuid,
    #[serde(flatten)]
    error: AppError,
    #[serde(skip_serializing_if = "Option::is_none")]
    index_url: Option<String>,
}

/// 검증 → 변환 → (후처리 컴파일) → 스트리밍. 협조적 취소(레지스트리
/// 플래그)를 청크 사이마다 확인. 결과는 동시에 `sink`(임시 NDJSON)에
/// 누적되어 `export_result` IPC에서 소비된다.
pub async fn run_query<R: Runtime>(
    app: tauri::AppHandle<R>,
    client: FirestoreClient,
    registry: Arc<StreamRegistry>,
    sink: Option<Arc<Mutex<ResultSink>>>,
    stream_id: String,
    session_id: Uuid,
    dsl: QueryDsl,
) {
    let chunk_ev = format!("query:chunk:{stream_id}");
    let done_ev = format!("query:done:{stream_id}");
    let err_ev = format!("query:error:{stream_id}");
    let started = Instant::now();

    // 누락 인덱스 안내 URL 등 보조 정보를 함께 전달하기 위해
    // (AppError, Option<index_url>) 튜플로 반환한다.
    let outcome: Result<(usize, usize, bool), (AppError, Option<String>)> = async {
        validate(&dsl).map_err(|e| (e, None))?;
        let params = translate(&dsl).map_err(|e| (e, None))?;
        // validate가 컴파일 가능성을 보장하므로 여기서는 실패하지 않는다.
        let matcher = dsl
            .post_filter
            .as_ref()
            .map(post_filter::compile)
            .transpose()
            .map_err(|e| (e, None))?;

        let collection_path = match &dsl.target {
            crate::query::dsl::QueryTarget::Collection { path } => path.as_str(),
            crate::query::dsl::QueryTarget::CollectionGroup { id } => id.as_str(),
        };
        tracing::info!(
            collection = %collection_path,
            stream_id = %stream_id,
            op = "query_start",
            "query started"
        );

        let mut stream = client
            .db
            .stream_query_doc_with_errors(params)
            .await
            .map_err(|err| {
                // 메시지 본문은 일반화 유지 + 인덱스 URL만 추출해 별도 필드로.
                let raw = format!("{err:?}");
                let url = extract_firestore_index_url(&raw);
                (
                    AppError::Firestore {
                        message: "failed to start query stream".into(),
                    },
                    url,
                )
            })?;

        let mut buf: Vec<Document> = Vec::with_capacity(PAGE);
        let mut page: u32 = 0;
        let mut matched: usize = 0;
        let mut scanned: usize = 0;
        let mut cancelled = false;

        while let Some(item) = stream.next().await {
            if registry.is_cancelled(&stream_id) {
                cancelled = true;
                break;
            }
            let item = item.map_err(|err| {
                let raw = format!("{err:?}");
                let url = extract_firestore_index_url(&raw);
                (
                    AppError::Firestore {
                        message: "error while streaming query results".into(),
                    },
                    url,
                )
            })?;
            let doc = decode_document(&item);
            scanned += 1;
            let is_matched = matcher.as_ref().is_none_or(|m| m.matches(&doc.data));
            // sink는 scanned 전체를 기록 (matched 플래그로 source 구분).
            if let Some(sink) = sink.as_ref() {
                if let Err(e) = sink.lock().append(&doc, is_matched) {
                    tracing::warn!(
                        target: "query",
                        error = %e,
                        stream_id = %stream_id,
                        "failed to append to result sink"
                    );
                }
            }
            // 후처리 통과 문서만 청크로 전달 (`docs/04-query-dsl.md`).
            if !is_matched {
                continue;
            }
            buf.push(doc);
            matched += 1;
            if buf.len() >= PAGE {
                let _ = app.emit(
                    &chunk_ev,
                    ChunkPayload {
                        session_id,
                        docs: std::mem::take(&mut buf),
                        page,
                    },
                );
                page += 1;
            }
        }
        if !buf.is_empty() {
            let _ = app.emit(
                &chunk_ev,
                ChunkPayload {
                    session_id,
                    docs: std::mem::take(&mut buf),
                    page,
                },
            );
        }
        Ok((matched, scanned, cancelled))
    }
    .await;

    registry.finish(&stream_id);

    match outcome {
        Ok((matched, scanned, cancelled)) => {
            let took_ms = started.elapsed().as_millis() as u64;
            tracing::info!(
                target: "query",
                count = matched,
                scanned,
                took_ms,
                stream_id = %stream_id,
                op = "query_done",
                "query finished"
            );
            let _ = app.emit(
                &done_ev,
                DonePayload {
                    session_id,
                    total: matched,
                    scanned,
                    took_ms,
                    has_more: cancelled,
                },
            );
        }
        Err((error, index_url)) => {
            let _ = app.emit(&err_ev, QueryErrorPayload { session_id, error, index_url });
        }
    }
}
