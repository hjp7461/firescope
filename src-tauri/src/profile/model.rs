//! 프로파일/자격증명 도메인 타입.
//!
//! 분리 원칙 (`docs/07-profiles.md`):
//! - [`Profile`]      : `profiles.json`에 영속화되는 **메타데이터**. 자격증명 본문 없음.
//! - [`ProfileMeta`]  : IPC로 프론트에 전달되는 마스킹 뷰 (`credential_ref` 대신 `has_credential`).
//! - [`Credential`]   : OS Vault ↔ 백엔드 사이에서만 흐르는 **비밀**. `SecretString`으로 보호.

use std::fmt;

use chrono::{DateTime, Utc};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 인증/접속 모드. 프로파일 단위로 적용된다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMode {
    Emulator,
    ServiceAccount,
    IdToken,
}

/// `profiles.json`에 저장되는 프로파일 메타데이터.
///
/// 자격증명 본문은 절대 포함하지 않는다. Vault에 자격증명이 있으면
/// `credential_ref`에 키 문자열(`profile:<id>`)만 보관한다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub project_id: String,
    pub mode: ProfileMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub firestore_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_host: Option<String>,

    pub require_confirmation: bool,
    pub read_only_warning: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_ref: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<DateTime<Utc>>,
    pub use_count: u64,
}

impl Profile {
    /// Vault 키 규칙: `profile:<uuid>` (`docs/07-profiles.md` "Vault 키 규칙").
    pub fn credential_account(&self) -> String {
        format!("profile:{}", self.id)
    }

    /// IPC로 내보낼 마스킹 뷰로 변환. `has_credential`은 Vault 조회 결과를
    /// 호출부(ProfileManager)가 주입한다 — Profile 자체는 Vault를 모른다.
    pub fn to_meta(&self, has_credential: bool) -> ProfileMeta {
        ProfileMeta {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            project_id: self.project_id.clone(),
            mode: self.mode,
            color: self.color.clone(),
            tags: self.tags.clone(),
            firestore_host: self.firestore_host.clone(),
            auth_host: self.auth_host.clone(),
            require_confirmation: self.require_confirmation,
            read_only_warning: self.read_only_warning,
            has_credential,
            created_at: self.created_at,
            last_used_at: self.last_used_at,
            use_count: self.use_count,
        }
    }
}

/// IPC 응답 전용. `credential_ref`가 `has_credential: bool`로 대체된다 —
/// 프론트는 자격증명의 존재 여부만 알 수 있고 키/본문은 알 수 없다.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileMeta {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub project_id: String,
    pub mode: ProfileMode,
    pub color: Option<String>,
    pub tags: Vec<String>,
    pub firestore_host: Option<String>,
    pub auth_host: Option<String>,
    pub require_confirmation: bool,
    pub read_only_warning: bool,
    pub has_credential: bool,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub use_count: u64,
}

/// `tauri-plugin-store`의 `profiles.json` 루트 스키마.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStoreData {
    pub version: u32,
    pub profiles: Vec<Profile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_profile_id: Option<Uuid>,
}

impl Default for ProfileStoreData {
    fn default() -> Self {
        Self {
            version: 1,
            profiles: Vec::new(),
            default_profile_id: None,
        }
    }
}

/// `create_profile` IPC 파라미터 (`docs/03-ipc-contract.md`).
#[derive(Debug, Clone, Deserialize)]
pub struct CreateProfileParams {
    pub name: String,
    pub description: Option<String>,
    pub project_id: String,
    pub mode: ProfileMode,
    pub color: Option<String>,
    pub tags: Option<Vec<String>>,
    pub firestore_host: Option<String>,
    pub auth_host: Option<String>,
    pub require_confirmation: Option<bool>,
    pub read_only_warning: Option<bool>,
}

/// `update_profile` IPC 파라미터. 자격증명 외 메타만 수정 (앱 로컬 데이터).
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateProfileParams {
    pub profile_id: Uuid,
    pub name: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
    pub tags: Option<Vec<String>>,
    pub firestore_host: Option<String>,
    pub auth_host: Option<String>,
    pub require_confirmation: Option<bool>,
    pub read_only_warning: Option<bool>,
}

/// Vault에서만 흐르는 자격증명 본문. `SecretString`이 메모리를 zeroize하고
/// `Debug`를 마스킹한다. **Serialize를 의도적으로 구현하지 않는다** — IPC
/// 응답·로그·`profiles.json` 어디에도 직렬화되지 못하게 컴파일 차원에서 차단.
#[derive(Clone)]
pub enum Credential {
    ServiceAccount { json: SecretString },
    IdToken { token: SecretString },
}

impl Credential {
    pub fn kind_str(&self) -> &'static str {
        match self {
            Credential::ServiceAccount { .. } => "service_account",
            Credential::IdToken { .. } => "id_token",
        }
    }
}

/// 본문이 절대 노출되지 않도록 손수 마스킹. `tracing`이나 `{:?}`에 실수로
/// 넘어가도 키 종류만 보인다.
impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credential")
            .field("kind", &self.kind_str())
            .field("body", &"<redacted>")
            .finish()
    }
}
