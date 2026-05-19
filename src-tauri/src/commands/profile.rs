//! 프로파일 관리 커맨드 (`docs/03-ipc-contract.md` §1).

use chrono::Utc;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::firestore::probe;
use crate::profile::{
    CreateProfileParams, Credential, ProfileMeta, ProfileMode, UpdateProfileParams,
};
use crate::state::AppState;

/// `set_credential` 입력. 본문 String은 즉시 `SecretString`으로 감싼다.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CredentialInput {
    ServiceAccount { json: String },
    IdToken { token: String },
}

impl From<CredentialInput> for Credential {
    fn from(input: CredentialInput) -> Self {
        match input {
            CredentialInput::ServiceAccount { json } => Credential::ServiceAccount {
                json: SecretString::from(json),
            },
            CredentialInput::IdToken { token } => Credential::IdToken {
                token: SecretString::from(token),
            },
        }
    }
}

#[derive(Serialize)]
pub struct CredentialStatus {
    pub has_credential: bool,
}

#[derive(Serialize)]
pub struct TestResult {
    pub ok: bool,
    pub project_id: String,
    pub latency_ms: u64,
}

#[derive(Serialize)]
pub struct ExportResult {
    pub written_bytes: u64,
    pub count: usize,
}

#[derive(Serialize)]
pub struct ImportDetail {
    pub name: String,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub details: Vec<ImportDetail>,
}

/// export/import 의 메타데이터 전용 표현 (자격증명 절대 미포함).
#[derive(Serialize, Deserialize)]
struct PortableProfile {
    name: String,
    project_id: String,
    mode: ProfileMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct PortableBundle {
    version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    exported_at: Option<String>,
    profiles: Vec<PortableProfile>,
}

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> AppResult<Vec<ProfileMeta>> {
    Ok(state.profiles.list())
}

#[tauri::command]
pub fn get_profile(state: State<'_, AppState>, profile_id: Uuid) -> AppResult<Option<ProfileMeta>> {
    Ok(state.profiles.get_meta(profile_id))
}

#[tauri::command]
pub fn create_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    params: CreateProfileParams,
) -> AppResult<ProfileMeta> {
    let meta = state.profiles.create(params)?;
    let _ = app.emit("profile:updated", meta.clone());
    Ok(meta)
}

#[tauri::command]
pub fn update_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    params: UpdateProfileParams,
) -> AppResult<ProfileMeta> {
    let meta = state.profiles.update(params)?;
    let _ = app.emit("profile:updated", meta.clone());
    Ok(meta)
}

#[tauri::command]
pub fn delete_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: Uuid,
) -> AppResult<()> {
    // 활성 프로파일을 지우면 세션도 함께 정리한다 (이벤트는 deactivate가 emit).
    if state
        .sessions
        .current()
        .is_some_and(|s| s.profile_id == profile_id)
    {
        state.sessions.deactivate(&app)?;
    }
    state.profiles.delete(profile_id)?;
    let _ = app.emit("profile:deleted", DeletedPayload { profile_id });
    Ok(())
}

#[derive(Serialize, Clone)]
struct DeletedPayload {
    profile_id: Uuid,
}

#[tauri::command]
pub fn duplicate_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: Uuid,
    new_name: String,
) -> AppResult<ProfileMeta> {
    let meta = state.profiles.duplicate(profile_id, new_name)?;
    let _ = app.emit("profile:updated", meta.clone());
    Ok(meta)
}

#[tauri::command]
pub fn set_credential(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: Uuid,
    credential: CredentialInput,
) -> AppResult<CredentialStatus> {
    state
        .profiles
        .set_credential(profile_id, credential.into())?;
    if let Some(meta) = state.profiles.get_meta(profile_id) {
        let _ = app.emit("profile:updated", meta);
    }
    Ok(CredentialStatus {
        has_credential: true,
    })
}

#[tauri::command]
pub fn clear_credential(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: Uuid,
) -> AppResult<CredentialStatus> {
    state.profiles.clear_credential(profile_id)?;
    if let Some(meta) = state.profiles.get_meta(profile_id) {
        let _ = app.emit("profile:updated", meta);
    }
    Ok(CredentialStatus {
        has_credential: false,
    })
}

/// 실제 활성화 없이 연결 가능 여부 검증. 모드별 검증 로직은
/// `firestore::probe`(도메인)에 있고 이 커맨드는 얇은 어댑터다 (원칙 4).
#[tauri::command]
pub async fn test_profile(state: State<'_, AppState>, profile_id: Uuid) -> AppResult<TestResult> {
    let profile = state
        .profiles
        .get_profile(profile_id)
        .ok_or_else(|| AppError::profile_not_found(format!("no profile with id {profile_id}")))?;
    let credential = state.profiles.credential(profile_id)?;
    let latency_ms = probe(&profile, credential).await?;
    Ok(TestResult {
        ok: true,
        project_id: profile.project_id,
        latency_ms,
    })
}

#[tauri::command]
pub fn export_profiles(
    state: State<'_, AppState>,
    profile_ids: Option<Vec<Uuid>>,
    path: String,
) -> AppResult<ExportResult> {
    let profiles: Vec<PortableProfile> = state
        .profiles
        .list_full()
        .into_iter()
        .filter(|p| profile_ids.as_ref().map_or(true, |ids| ids.contains(&p.id)))
        .map(|p| PortableProfile {
            name: p.name,
            project_id: p.project_id,
            mode: p.mode,
            color: p.color,
            tags: p.tags,
        })
        .collect();

    let count = profiles.len();
    let bundle = PortableBundle {
        version: 1,
        exported_at: Some(Utc::now().to_rfc3339()),
        profiles,
    };
    let json = serde_json::to_vec_pretty(&bundle)
        .map_err(|_| AppError::internal("failed to serialize profile bundle"))?;
    std::fs::write(&path, &json)
        .map_err(|e| AppError::io(format!("failed to write export file: {e}")))?;

    Ok(ExportResult {
        written_bytes: json.len() as u64,
        count,
    })
}

#[tauri::command]
pub fn import_profiles(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> AppResult<ImportResult> {
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| AppError::io(format!("failed to read import file: {e}")))?;
    let bundle: PortableBundle = serde_json::from_str(&raw)
        .map_err(|_| AppError::io("import file is not a valid profile bundle"))?;

    let mut imported = 0;
    let mut skipped = 0;
    let mut details = Vec::with_capacity(bundle.profiles.len());

    for entry in bundle.profiles {
        let name = entry.name.clone();
        let params = CreateProfileParams {
            name: entry.name,
            description: None,
            project_id: entry.project_id,
            mode: entry.mode,
            color: entry.color,
            tags: Some(entry.tags),
            firestore_host: None,
            auth_host: None,
            require_confirmation: None,
            read_only_warning: None,
        };
        match state.profiles.create(params) {
            Ok(meta) => {
                imported += 1;
                let _ = app.emit("profile:updated", meta);
                details.push(ImportDetail {
                    name,
                    status: "imported",
                    reason: None,
                });
            }
            Err(e) => {
                skipped += 1;
                details.push(ImportDetail {
                    name,
                    status: "skipped",
                    reason: Some(e.to_string()),
                });
            }
        }
    }

    Ok(ImportResult {
        imported,
        skipped,
        details,
    })
}
