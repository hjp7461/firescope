//! 프로파일 메타데이터의 단일 진입점.
//!
//! 영속화는 `tauri-plugin-store`(`profiles.json`)에 위임하고, 읽기는
//! 인메모리 캐시로 빠르게 처리한다. 자격증명 본문은 절대 다루지 않으며
//! [`CredentialVault`] 키 생성/삭제만 조율한다.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use parking_lot::RwLock;
use tauri::Runtime;
use tauri_plugin_store::Store;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::profile::model::{
    CreateProfileParams, Credential, Profile, ProfileMeta, ProfileMode, ProfileStoreData,
    UpdateProfileParams,
};
use crate::profile::vault::CredentialVault;

/// `profiles.json` 안에서 전체 데이터를 담는 단일 키.
const DATA_KEY: &str = "data";

pub struct ProfileManager<R: Runtime> {
    store: Arc<Store<R>>,
    vault: CredentialVault,
    cache: RwLock<HashMap<Uuid, Profile>>,
}

impl<R: Runtime> ProfileManager<R> {
    /// 기존 `profiles.json`을 읽어 캐시를 채운다. 파일이 없거나 비어 있으면
    /// 빈 상태로 시작한다.
    pub fn load(store: Arc<Store<R>>, vault: CredentialVault) -> AppResult<Self> {
        let data: ProfileStoreData = match store.get(DATA_KEY) {
            Some(value) => serde_json::from_value(value).map_err(|_| {
                AppError::io("profiles.json is corrupt or has an incompatible schema")
            })?,
            None => ProfileStoreData::default(),
        };

        let cache = data.profiles.into_iter().map(|p| (p.id, p)).collect();
        Ok(Self {
            store,
            vault,
            cache: RwLock::new(cache),
        })
    }

    // ── 조회 ────────────────────────────────────────────────────────────

    /// IPC용 마스킹 목록. 생성 시각 기준 정렬로 안정적인 순서를 보장한다.
    pub fn list(&self) -> Vec<ProfileMeta> {
        let cache = self.cache.read();
        let mut metas: Vec<ProfileMeta> = cache
            .values()
            .map(|p| p.to_meta(self.vault.has(p.id)))
            .collect();
        metas.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        metas
    }

    pub fn get_meta(&self, id: Uuid) -> Option<ProfileMeta> {
        let cache = self.cache.read();
        cache.get(&id).map(|p| p.to_meta(self.vault.has(id)))
    }

    /// 내부 계층(세션 활성화 등)이 쓰는 전체 Profile. IPC로 내보내지 않는다.
    pub fn get_profile(&self, id: Uuid) -> Option<Profile> {
        self.cache.read().get(&id).cloned()
    }

    /// 세션 활성화 시 자격증명 조회. Vault를 캡슐화한 채로 내부 계층에만
    /// 노출한다 — 반환된 `Credential`은 로그/IPC로 새지 않아야 한다.
    pub fn credential(&self, id: Uuid) -> AppResult<Option<Credential>> {
        self.vault.get(id)
    }

    /// export 용 전체 스냅샷. 자격증명 본문은 애초에 Profile에 없다.
    pub fn list_full(&self) -> Vec<Profile> {
        self.cache.read().values().cloned().collect()
    }

    /// 자격증명 등록/갱신. **저장 전 형식 검증**(본문 미노출) 후 Vault에만
    /// 본문을 저장하고, 메타에는 `credential_ref`(키 문자열)만 남긴다.
    pub fn set_credential(&self, id: Uuid, cred: Credential) -> AppResult<()> {
        crate::profile::validation::validate(&cred)?;
        let account = {
            let cache = self.cache.read();
            let profile = cache
                .get(&id)
                .ok_or_else(|| AppError::profile_not_found(format!("no profile with id {id}")))?;
            profile.credential_account()
        };
        self.vault.set(id, &cred)?;
        if let Some(profile) = self.cache.write().get_mut(&id) {
            profile.credential_ref = Some(account);
        }
        self.persist()
    }

    /// 자격증명만 제거 (프로파일 메타는 유지).
    pub fn clear_credential(&self, id: Uuid) -> AppResult<()> {
        if !self.cache.read().contains_key(&id) {
            return Err(AppError::profile_not_found(format!(
                "no profile with id {id}"
            )));
        }
        self.vault.remove(id)?;
        if let Some(profile) = self.cache.write().get_mut(&id) {
            profile.credential_ref = None;
        }
        self.persist()
    }

    /// Vault에 자격증명이 존재하는지 (test/메타 표시용).
    pub fn has_credential(&self, id: Uuid) -> bool {
        self.vault.has(id)
    }

    // ── 변경 ────────────────────────────────────────────────────────────

    pub fn create(&self, params: CreateProfileParams) -> AppResult<ProfileMeta> {
        self.ensure_name_unique(&params.name, None)?;

        // 운영 환경 자동 보호: service_account + project_id가 prod-스러우면
        // 사용자가 명시하지 않은 한 확인/경고를 강제한다.
        let looks_prod = params.mode == ProfileMode::ServiceAccount
            && project_id_looks_production(&params.project_id);

        let id = Uuid::new_v4();
        let profile = Profile {
            id,
            name: params.name.trim().to_owned(),
            description: params.description,
            project_id: params.project_id,
            mode: params.mode,
            color: params.color,
            tags: params.tags.unwrap_or_default(),
            firestore_host: params.firestore_host,
            auth_host: params.auth_host,
            require_confirmation: params.require_confirmation.unwrap_or(looks_prod),
            read_only_warning: params.read_only_warning.unwrap_or(looks_prod),
            credential_ref: None,
            created_at: Utc::now(),
            last_used_at: None,
            use_count: 0,
        };

        self.cache.write().insert(id, profile);
        self.persist()?;
        tracing::info!(target: "profile", profile_id = %id, "profile created");
        self.get_meta(id)
            .ok_or_else(|| AppError::internal("created profile vanished from cache"))
    }

    pub fn update(&self, params: UpdateProfileParams) -> AppResult<ProfileMeta> {
        let id = params.profile_id;
        if let Some(new_name) = &params.name {
            self.ensure_name_unique(new_name, Some(id))?;
        }

        {
            let mut cache = self.cache.write();
            let profile = cache
                .get_mut(&id)
                .ok_or_else(|| AppError::profile_not_found(format!("no profile with id {id}")))?;

            if let Some(v) = params.name {
                profile.name = v.trim().to_owned();
            }
            if let Some(v) = params.description {
                profile.description = Some(v);
            }
            if let Some(v) = params.color {
                profile.color = Some(v);
            }
            if let Some(v) = params.tags {
                profile.tags = v;
            }
            if let Some(v) = params.firestore_host {
                profile.firestore_host = Some(v);
            }
            if let Some(v) = params.auth_host {
                profile.auth_host = Some(v);
            }
            if let Some(v) = params.require_confirmation {
                profile.require_confirmation = v;
            }
            if let Some(v) = params.read_only_warning {
                profile.read_only_warning = v;
            }
        }

        self.persist()?;
        tracing::info!(target: "profile", profile_id = %id, "profile updated");
        self.get_meta(id)
            .ok_or_else(|| AppError::profile_not_found(format!("no profile with id {id}")))
    }

    /// 프로파일과 **연결된 자격증명을 Vault에서 함께** 제거.
    pub fn delete(&self, id: Uuid) -> AppResult<()> {
        let removed = self.cache.write().remove(&id);
        if removed.is_none() {
            return Err(AppError::profile_not_found(format!(
                "no profile with id {id}"
            )));
        }
        // 메타 제거가 성공했으면 자격증명도 정리. Vault 실패는 치명적이지 않지만
        // 호출부가 알 수 있도록 전파한다(고아 자격증명 방지).
        self.vault.remove(id)?;
        self.persist()?;
        tracing::info!(target: "profile", profile_id = %id, "profile deleted");
        Ok(())
    }

    /// 자격증명은 복제하지 않는다 — 사용자가 새 프로파일에 다시 입력해야 한다.
    pub fn duplicate(&self, id: Uuid, new_name: String) -> AppResult<ProfileMeta> {
        self.ensure_name_unique(&new_name, None)?;

        let source = self
            .get_profile(id)
            .ok_or_else(|| AppError::profile_not_found(format!("no profile with id {id}")))?;

        let new_id = Uuid::new_v4();
        let clone = Profile {
            id: new_id,
            name: new_name.trim().to_owned(),
            credential_ref: None,
            created_at: Utc::now(),
            last_used_at: None,
            use_count: 0,
            ..source
        };

        self.cache.write().insert(new_id, clone);
        self.persist()?;
        tracing::info!(target: "profile", profile_id = %new_id, source_id = %id, "profile duplicated");
        self.get_meta(new_id)
            .ok_or_else(|| AppError::internal("duplicated profile vanished from cache"))
    }

    // ── 내부 ────────────────────────────────────────────────────────────

    /// 이름 중복 검증 (대소문자/공백 무시). `exclude`는 update/자기 자신용.
    fn ensure_name_unique(&self, name: &str, exclude: Option<Uuid>) -> AppResult<()> {
        let needle = name.trim().to_lowercase();
        let clash = self
            .cache
            .read()
            .values()
            .any(|p| Some(p.id) != exclude && p.name.trim().to_lowercase() == needle);
        if clash {
            Err(AppError::duplicate_profile(format!(
                "a profile named \"{name}\" already exists"
            )))
        } else {
            Ok(())
        }
    }

    /// 캐시 전체를 `profiles.json`에 직렬화 후 디스크에 flush.
    fn persist(&self) -> AppResult<()> {
        let snapshot = ProfileStoreData {
            version: 1,
            profiles: self.cache.read().values().cloned().collect(),
            default_profile_id: None,
        };
        let value = serde_json::to_value(&snapshot)
            .map_err(|_| AppError::internal("failed to serialize profile store"))?;
        self.store.set(DATA_KEY, value);
        self.store
            .save()
            .map_err(|e| AppError::io(format!("failed to write profiles.json: {e}")))
    }
}

/// 운영 프로젝트 휴리스틱. `prod`/`production`이 프로젝트 ID에 포함되면
/// 운영으로 간주한다 (보수적 기본값 — 사용자가 명시 옵션으로 덮어쓸 수 있음).
fn project_id_looks_production(project_id: &str) -> bool {
    let p = project_id.to_lowercase();
    p.contains("prod")
}
