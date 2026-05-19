//! Firestore 연결/데이터플레인 계층.
//!
//! Phase 2부터 활성화 시점에 라이브 `FirestoreDb`를 생성하고
//! (`connection`), protobuf 결과를 DSL로 디코드(`decode`)하며,
//! 페이지네이션 스트리밍(`streaming`)으로 결과를 이벤트 전송한다.

pub mod connection;
pub mod decode;
pub mod result_sink;
pub mod streaming;

pub use connection::{probe, FirestoreClient};
pub use decode::{decode_document, Document};
pub use result_sink::{ExportFormat, ExportSource, ResultSink};
