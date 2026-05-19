//! 애플리케이션 전역 에러 타입.
//!
//! 보안 invariant: 이 enum의 어떤 variant 메시지에도 자격증명 본문
//! (서비스 계정 JSON, ID 토큰)을 절대 포함하지 않는다. 검증 실패 시에도
//! `"invalid service account JSON"` 같은 일반 메시지만 사용한다.

use serde::Serialize;

pub type AppResult<T> = std::result::Result<T, AppError>;

/// IPC 경계를 넘어 프론트로 전달되는 에러.
///
/// `#[serde(tag = "kind")]`로 직렬화되어 프론트의 `AppError` 유니온 타입
/// (`docs/03-ipc-contract.md`)과 1:1 대응한다. 프론트는 `kind`로 분기한다
/// (예: `confirmation_required` → 운영 경고 모달 후 재시도).
#[derive(Debug, Clone, thiserror::Error, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppError {
    #[error("auth: {message}")]
    Auth { message: String },

    #[error("firestore: {message}")]
    Firestore { message: String },

    #[error("query invalid: {message}")]
    InvalidQuery { message: String },

    #[error("io: {message}")]
    Io { message: String },

    #[error("internal: {message}")]
    Internal { message: String },

    #[error("no active session")]
    NoSession { message: String },

    #[error("profile not found")]
    ProfileNotFound { message: String },

    #[error("credential not found")]
    CredentialNotFound { message: String },

    #[error("credential invalid")]
    CredentialInvalid { message: String },

    #[error("confirmation required")]
    ConfirmationRequired { message: String },

    #[error("vault error")]
    VaultError { message: String },

    #[error("duplicate profile")]
    DuplicateProfile { message: String },
}

impl AppError {
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::Io {
            message: message.into(),
        }
    }

    pub fn profile_not_found(message: impl Into<String>) -> Self {
        Self::ProfileNotFound {
            message: message.into(),
        }
    }

    pub fn credential_not_found(message: impl Into<String>) -> Self {
        Self::CredentialNotFound {
            message: message.into(),
        }
    }

    /// 자격증명 검증 실패. 호출부는 절대 본문/토큰 조각을 message에 넣지 않는다.
    pub fn credential_invalid(message: impl Into<String>) -> Self {
        Self::CredentialInvalid {
            message: message.into(),
        }
    }

    pub fn duplicate_profile(message: impl Into<String>) -> Self {
        Self::DuplicateProfile {
            message: message.into(),
        }
    }

    /// OS Vault(keyring) 실패. keyring 에러는 자격증명 본문을 담지 않으므로
    /// 일반화된 컨텍스트 문자열만 전달한다.
    pub fn vault(context: impl Into<String>) -> Self {
        Self::VaultError {
            message: context.into(),
        }
    }
}
