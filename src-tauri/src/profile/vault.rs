//! 자격증명 저장소 포트(trait)와 OS Keyring 어댑터.
//!
//! 원칙 4·7: 도메인(`ProfileManager`)은 [`CredentialStore`] trait에만
//! 의존한다. 실제 OS 자격증명 저장소는 [`OsKeyringVault`]가 캡슐화하며,
//! 테스트는 [`InMemoryVault`]를 주입한다. 자격증명 본문은 평문 파일/
//! 로그/에러로 새지 않는다.

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

/// 자격증명 저장소 포트. 도메인은 이 trait에만 의존한다 (mock 주입 가능).
pub trait CredentialStore: Send + Sync {
    fn set(&self, profile_id: Uuid, cred: &Credential) -> AppResult<()>;
    fn get(&self, profile_id: Uuid) -> AppResult<Option<Credential>>;
    fn remove(&self, profile_id: Uuid) -> AppResult<()>;
    fn has(&self, profile_id: Uuid) -> bool;
}

/// Vault에 직렬화되어 저장되는 본문 형식.
///
/// `serde_json::Value`를 거치지 않고 이 구조체로 직접 (역)직렬화하여
/// 비밀이 머무는 중간 표현을 최소화한다. 필드는 사용 직후 zeroize 한다.
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

fn to_envelope(cred: &Credential) -> CredentialEnvelope {
    match cred {
        Credential::ServiceAccount { json } => CredentialEnvelope::ServiceAccount {
            json: json.expose_secret().to_owned(),
        },
        Credential::IdToken { token } => CredentialEnvelope::IdToken {
            token: token.expose_secret().to_owned(),
        },
    }
}

fn from_envelope(envelope: &CredentialEnvelope) -> Credential {
    match envelope {
        CredentialEnvelope::ServiceAccount { json } => Credential::ServiceAccount {
            json: SecretString::from(json.clone()),
        },
        CredentialEnvelope::IdToken { token } => Credential::IdToken {
            token: SecretString::from(token.clone()),
        },
    }
}

/// OS 자격증명 저장소(keyring) 어댑터. 상태가 없고 연산마다 Entry를 연다.
#[derive(Debug, Clone, Copy, Default)]
pub struct OsKeyringVault;

impl OsKeyringVault {
    pub fn new() -> Self {
        Self
    }

    fn entry(profile_id: Uuid) -> AppResult<Entry> {
        Entry::new(SERVICE_NAME, &account_for(profile_id))
            .map_err(|e| AppError::vault(format!("cannot open keyring entry: {e}")))
    }
}

impl CredentialStore for OsKeyringVault {
    fn set(&self, profile_id: Uuid, cred: &Credential) -> AppResult<()> {
        let entry = Self::entry(profile_id)?;
        let envelope = to_envelope(cred);

        // 직렬화 결과 문자열도 비밀이다 — 사용 후 zeroize.
        let mut serialized = serde_json::to_string(&envelope)
            .map_err(|_| AppError::vault("failed to serialize credential envelope"))?;
        drop(envelope); // 내부 String zeroize

        let result = entry
            .set_password(&serialized)
            .map_err(|e| AppError::vault(format!("failed to store credential: {e}")));
        serialized.zeroize();
        result?;
        tracing::info!(target: "vault", profile_id = %profile_id, "credential stored");
        Ok(())
    }

    fn get(&self, profile_id: Uuid) -> AppResult<Option<Credential>> {
        let entry = Self::entry(profile_id)?;
        let mut raw = match entry.get_password() {
            Ok(s) => s,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(e) => return Err(AppError::vault(format!("failed to read credential: {e}"))),
        };
        let envelope: CredentialEnvelope = serde_json::from_str(&raw)
            .map_err(|_| AppError::vault("stored credential envelope is corrupt"))?;
        raw.zeroize();
        let cred = from_envelope(&envelope);
        drop(envelope);
        Ok(Some(cred))
    }

    fn remove(&self, profile_id: Uuid) -> AppResult<()> {
        let entry = Self::entry(profile_id)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {
                tracing::info!(target: "vault", profile_id = %profile_id, "credential removed");
                Ok(())
            }
            Err(e) => Err(AppError::vault(format!("failed to remove credential: {e}"))),
        }
    }

    fn has(&self, profile_id: Uuid) -> bool {
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

/// 테스트용 인메모리 자격증명 저장소 (원칙 7).
#[cfg(test)]
pub struct InMemoryVault {
    inner: parking_lot::Mutex<std::collections::HashMap<Uuid, Credential>>,
}

#[cfg(test)]
impl InMemoryVault {
    pub fn new() -> Self {
        Self {
            inner: parking_lot::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[cfg(test)]
impl CredentialStore for InMemoryVault {
    fn set(&self, profile_id: Uuid, cred: &Credential) -> AppResult<()> {
        self.inner.lock().insert(profile_id, cred.clone());
        Ok(())
    }
    fn get(&self, profile_id: Uuid) -> AppResult<Option<Credential>> {
        Ok(self.inner.lock().get(&profile_id).cloned())
    }
    fn remove(&self, profile_id: Uuid) -> AppResult<()> {
        self.inner.lock().remove(&profile_id);
        Ok(())
    }
    fn has(&self, profile_id: Uuid) -> bool {
        self.inner.lock().contains_key(&profile_id)
    }
}
