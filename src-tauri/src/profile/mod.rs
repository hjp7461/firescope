//! 프로파일/자격증명 계층.
//!
//! - [`model`]      : 도메인 타입 (`Profile`, `ProfileMeta`, `Credential`)
//! - [`store`]      : `ProfileManager` — 메타데이터 영속화 + CRUD
//! - [`vault`]      : `CredentialVault` — OS 자격증명 저장소
//! - [`validation`] : 저장 전 자격증명 형식 검증

pub mod model;
pub mod store;
pub mod validation;
pub mod vault;

// Phase 1-D(IPC 커맨드)에서 소비된다. 그 전까지는 미사용 경고를 억제.
#[allow(unused_imports)]
pub use model::{
    CreateProfileParams, Credential, Profile, ProfileMeta, ProfileMode, UpdateProfileParams,
};
pub use store::ProfileManager;
pub use vault::CredentialVault;
