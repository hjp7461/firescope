//! 쿼리 DSL 계층 (순수 도메인).
//!
//! - [`dsl`]      : 프론트 JSON DSL 타입 (`docs/04-query-dsl.md`)
//! - [`validate`] : Firestore 제약 사전 검증 (7규칙)
//! - `translate`  : DSL → firestore `FirestoreQueryParams` (다음 증분)

pub mod dsl;
pub mod translate;
pub mod validate;

#[allow(unused_imports)]
pub use dsl::QueryDsl;
pub use translate::translate;
pub use validate::validate;
