//! Firestore 연결 계층.
//!
//! 1-C 범위: 프로파일 모드별 **연결 설정 해석**과 세션 보관.
//! 실제 gRPC `FirestoreDb` 핸들 생성과 쿼리/스트리밍은 Phase 2
//! (`docs/06-roadmap.md` Phase 2, `streaming.rs`)에서 이 설정을 입력으로
//! 구성한다 — 1-C에서는 데이터플레인 RPC를 발생시키지 않는다.

pub mod connection;

pub use connection::{probe, FirestoreClient};
