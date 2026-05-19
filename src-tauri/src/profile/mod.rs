//! 프로파일/자격증명 계층 (순수 도메인 — `tauri::*` 미import).
//!
//! - [`model`]      : 도메인 타입 (`Profile`, `ProfileMeta`, `Credential`)
//! - [`repository`] : `ProfileRepository` 포트 (영속화 추상화)
//! - [`vault`]      : `CredentialStore` 포트 + `OsKeyringVault` 어댑터
//! - [`store`]      : `ProfileManager` — 도메인 로직 (포트에만 의존)
//! - [`validation`] : 저장 전 자격증명 형식 검증

pub mod model;
pub mod repository;
pub mod store;
pub mod validation;
pub mod vault;

#[allow(unused_imports)]
pub use model::{
    CreateProfileParams, Credential, Profile, ProfileMeta, ProfileMode, ProfileStoreData,
    UpdateProfileParams,
};
pub use repository::ProfileRepository;
pub use store::ProfileManager;
pub use vault::{CredentialStore, OsKeyringVault};
