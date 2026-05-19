//! 쿼리 스트리밍 — 페이지 단위로 이벤트 전송 (원칙 6: 스트리밍 우선).
//!
//! `query_documents`는 즉시 반환하고, 결과는
//! `query:chunk:<sid>` / `query:done:<sid>` / `query:error:<sid>`
//! 이벤트로 전달한다 (`docs/03-ipc-contract.md` §4).

use std::sync::Arc;
use std::time::Instant;

use firestore::FirestoreQuerySupport;
use futures::StreamExt;
use serde::Serialize;
use tauri::{Emitter, Runtime};

use crate::error::{AppError, AppResult};
use crate::firestore::{decode_document, Document, FirestoreClient};
use crate::query::dsl::QueryDsl;
use crate::query::{post_filter, translate, validate};
use crate::state::StreamRegistry;

const PAGE: usize = 100;

#[derive(Serialize, Clone)]
struct ChunkPayload {
    docs: Vec<Document>,
    page: u32,
}

#[derive(Serialize, Clone)]
struct DonePayload {
    /// 후처리 통과(매칭) 문서 수 = chunk로 전달된 합계.
    total: usize,
    /// Firestore에서 가져온 전체 문서 수 (post_filter 없으면 total과 동일).
    scanned: usize,
    took_ms: u64,
    has_more: bool,
}

/// 검증 → 변환 → (후처리 컴파일) → 스트리밍. 협조적 취소(레지스트리
/// 플래그)를 청크 사이마다 확인.
pub async fn run_query<R: Runtime>(
    app: tauri::AppHandle<R>,
    client: FirestoreClient,
    registry: Arc<StreamRegistry>,
    stream_id: String,
    dsl: QueryDsl,
) {
    let chunk_ev = format!("query:chunk:{stream_id}");
    let done_ev = format!("query:done:{stream_id}");
    let err_ev = format!("query:error:{stream_id}");
    let started = Instant::now();

    let outcome: AppResult<(usize, usize, bool)> = async {
        validate(&dsl)?;
        let params = translate(&dsl)?;
        // validate가 컴파일 가능성을 보장하므로 여기서는 실패하지 않는다.
        let matcher = dsl
            .post_filter
            .as_ref()
            .map(post_filter::compile)
            .transpose()?;

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
            .map_err(|_| AppError::Firestore {
                message: "failed to start query stream".into(),
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
            let item = item.map_err(|_| AppError::Firestore {
                message: "error while streaming query results".into(),
            })?;
            let doc = decode_document(&item);
            scanned += 1;
            // 후처리 통과 문서만 청크로 전달 (`docs/04-query-dsl.md`).
            if matcher.as_ref().is_some_and(|m| !m.matches(&doc.data)) {
                continue;
            }
            buf.push(doc);
            matched += 1;
            if buf.len() >= PAGE {
                let _ = app.emit(
                    &chunk_ev,
                    ChunkPayload {
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
                    total: matched,
                    scanned,
                    took_ms,
                    has_more: cancelled,
                },
            );
        }
        Err(e) => {
            let _ = app.emit(&err_ev, e);
        }
    }
}
