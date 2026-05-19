//! 쿼리 DSL 계층 (순수 도메인).
//!
//! - [`dsl`]      : 프론트 JSON DSL 타입 (`docs/04-query-dsl.md`)
//! - [`validate`] : Firestore 제약 사전 검증 (8규칙)
//! - [`translate`]: DSL → firestore `FirestoreQueryParams` (전체 연산자/커서)
//! - [`history`]  : 프로파일별 쿼리 히스토리 영속화 포트/매니저

pub mod dsl;
pub mod history;
pub mod translate;
pub mod validate;

#[allow(unused_imports)]
pub use dsl::QueryDsl;
pub use translate::translate;
pub use validate::validate;
