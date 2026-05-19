//! OS 자격증명 저장소 추상화 (`keyring` 3.x).
//!
//! 자격증명 본문은 **백엔드 ↔ OS Vault** 사이에서만 흐른다. 이 모듈은
//! 본문을 평문 파일에 쓰지 않으며, 로그/에러에도 노출하지 않는다.
//!
//! 키 규칙 (`docs/07-profiles.md`):
//! - service: `com.firescope.credentials`
//! - account: `profile:<profile_id>`
//! - secret : 아래 [`CredentialEnvelope`] JSON

use keyring::Entry;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::error::{AppError, AppResult};
use crate::profile::model::Credential;

const SERVICE_NAME: &str = "com.firescope.credentials";

fn account_for(profile_id: Uuid) -> String {
    format!("profile:{profile_id}")
}

/// Vault에 직렬화되어 저장되는 본문 형식.
///
/// `serde_json::Value`를 거치지 않고 이 구조체로 직접 (역)직렬화하여
/// 비밀이 머무는 중간 표현을 최소화한다. `field`들은 사용 직후 zeroize 한다.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum CredentialEnvelope {
    ServiceAccount { json: String },
    IdToken { token: String },
}

impl Drop for CredentialEnvelope {
    fn drop(&mut self) {
        match self {
            CredentialEnvelope::ServiceAccount { json } => json.zeroize(),
            CredentialEnvelope::IdToken { token } => token.zeroize(),
        }
    }
}

/// OS 자격증명 저장소 핸들. 상태가 없고 (서비스명만 보유) 연산마다
/// `keyring::Entry`를 새로 연다.
#[derive(Debug, Clone, Copy)]
pub struct CredentialVault;

impl CredentialVault {
    pub fn new() -> Self {
        Self
    }

    fn entry(profile_id: Uuid) -> AppResult<Entry> {
        Entry::new(SERVICE_NAME, &account_for(profile_id))
            .map_err(|e| AppError::vault(format!("cannot open keyring entry: {e}")))
    }

    /// 자격증명을 OS Vault에 저장 (덮어쓰기). 본문은 메시지/로그로 새지 않는다.
    pub fn set(&self, profile_id: Uuid, cred: &Credential) -> AppResult<()> {
        let entry = Self::entry(profile_id)?;

        let envelope = match cred {
            Credential::ServiceAccount { json } => CredentialEnvelope::ServiceAccount {
                json: json.expose_secret().to_owned(),
            },
            Credential::IdToken { token } => CredentialEnvelope::IdToken {
                token: token.expose_secret().to_owned(),
            },
        };

        // 직렬화 결과 문자열도 비밀이다 — 사용 후 zeroize.
        let mut serialized = serde_json::to_string(&envelope)
            .map_err(|_| AppError::vault("failed to serialize credential envelope"))?;
        // envelope는 여기서 drop되며 내부 String이 zeroize된다.
        drop(envelope);

        let result = entry
            .set_password(&serialized)
            .map_err(|e| AppError::vault(format!("failed to store credential: {e}")));

        serialized.zeroize();
        result?;
        tracing::info!(target: "vault", profile_id = %profile_id, "credential stored");
        Ok(())
    }

    /// 자격증명 조회. 없으면 `Ok(None)`.
    pub fn get(&self, profile_id: Uuid) -> AppResult<Option<Credential>> {
        let entry = Self::entry(profile_id)?;

        let mut raw = match entry.get_password() {
            Ok(s) => s,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(e) => return Err(AppError::vault(format!("failed to read credential: {e}"))),
        };

        let envelope: CredentialEnvelope = serde_json::from_str(&raw)
            .map_err(|_| AppError::vault("stored credential envelope is corrupt"))?;
        raw.zeroize();

        let cred = match &envelope {
            CredentialEnvelope::ServiceAccount { json } => Credential::ServiceAccount {
                json: SecretString::from(json.clone()),
            },
            CredentialEnvelope::IdToken { token } => Credential::IdToken {
                token: SecretString::from(token.clone()),
            },
        };
        // envelope drop → 내부 평문 String zeroize.
        drop(envelope);

        Ok(Some(cred))
    }

    /// 자격증명 제거. 없는 경우도 성공으로 간주 (idempotent).
    pub fn remove(&self, profile_id: Uuid) -> AppResult<()> {
        let entry = Self::entry(profile_id)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {
                tracing::info!(target: "vault", profile_id = %profile_id, "credential removed");
                Ok(())
            }
            Err(e) => Err(AppError::vault(format!("failed to remove credential: {e}"))),
        }
    }

    /// 자격증명 존재 여부. 조회 에러는 "없음"으로 보수적으로 처리한다.
    pub fn has(&self, profile_id: Uuid) -> bool {
        match Self::entry(profile_id).and_then(|e| {
            e.get_password().map(|_| true).or_else(|err| match err {
                keyring::Error::NoEntry => Ok(false),
                other => Err(AppError::vault(format!("keyring probe failed: {other}"))),
            })
        }) {
            Ok(present) => present,
            Err(_) => {
                tracing::debug!(target: "vault", profile_id = %profile_id, "has() probe failed; treating as absent");
                false
            }
        }
    }
}

impl Default for CredentialVault {
    fn default() -> Self {
        Self::new()
    }
}
