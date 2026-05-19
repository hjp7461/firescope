//! Tauri 어댑터 계층 (원칙 4: 외부 시스템/프레임워크 결합은 여기로 격리).
//!
//! 도메인(`profile`/`auth`)은 포트 trait에만 의존하고, 이 모듈이 Tauri
//! 구현을 제공한다. `tauri::*` import는 이 계층과 `state.rs`(합성 루트)에만
//! 허용된다.

pub mod event_sink;
pub mod log_layer;
pub mod profile_repo;

pub use event_sink::TauriTokenSink;
pub use profile_repo::TauriProfileRepository;
