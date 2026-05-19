//! `#[tauri::command]` 모음 (`docs/03-ipc-contract.md` 계약 구현).
//!
//! 모든 커맨드는 `Result<T, AppError>`를 반환하며, 자격증명 본문은
//! 응답·로그·에러 어디에도 포함하지 않는다.

pub mod profile;
pub mod session;
