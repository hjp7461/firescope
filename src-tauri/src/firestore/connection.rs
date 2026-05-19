//! 모드별 라이브 Firestore 연결.
//!
//! Phase 2부터 활성화 시점에 실제 [`firestore::FirestoreDb`] gRPC 채널을
//! 생성한다 (1-C의 지연 경계 해제). 모드별 토큰 소스:
//! - service_account → `TokenSourceType::Json` (gcloud_sdk가 갱신까지 처리)
//! - 에뮬레이터/id_token → `ExternalSource`(클로저 기반, 무인증/토큰주입)
//!
//! 보안: 자격증명 본문은 토큰 소스 구성 지점에서만 노출하고 로그/에러로
//! 새지 않는다 (gcp_auth/gcloud 에러는 일반 메시지로 치환).

use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use chrono::{TimeDelta, Utc};
use firestore::{FirestoreDb, FirestoreDbOptions};
use gcloud_sdk::{ExternalJwtFunctionSource, SecretValue, Token, TokenSourceType};
use secrecy::ExposeSecret;

use crate::auth::{ServiceAccountAuth, FIRESTORE_SCOPES};
use crate::error::{AppError, AppResult};
use crate::profile::{Credential, Profile, ProfileMode};

/// 기본 에뮬레이터 호스트 (`docs/02-architecture.md` 우선순위 3순위).
const DEFAULT_EMULATOR_HOST: &str = "localhost:8080";
const EMULATOR_HOST_ENV: &str = "FIRESTORE_EMULATOR_HOST";

/// 활성 세션이 보유하는 라이브 Firestore 클라이언트.
/// `FirestoreDb`는 내부가 `Arc`라 clone이 값싸다.
#[derive(Clone)]
pub struct FirestoreClient {
    pub project_id: String,
    pub mode: ProfileMode,
    pub db: FirestoreDb,
}

/// 에뮬레이터 호스트 해석 (profile.firestore_host > env > 기본값) → `http://host`.
fn emulator_url(profile: &Profile) -> String {
    let host = profile
        .firestore_host
        .clone()
        .or_else(|| std::env::var(EMULATOR_HOST_ENV).ok())
        .unwrap_or_else(|| DEFAULT_EMULATOR_HOST.to_string());
    if host.contains("://") {
        host
    } else {
        format!("http://{host}")
    }
}

/// 무인증/정적 토큰을 공급하는 외부 토큰 소스.
///
/// 에뮬레이터는 토큰을 무시하므로 더미("owner"), id_token 모드는 사용자
/// 토큰을 그대로 Bearer로 주입한다. `ExternalJwtFunctionSource`는 클로저
/// 기반 공개 API라 `Source` trait을 직접 구현할 필요가 없다.
fn static_token_source(token: String) -> TokenSourceType {
    let make = move || {
        let tok = token.clone();
        async move {
            Ok::<_, gcloud_sdk::error::Error>(Token::new(
                "Bearer".to_string(),
                SecretValue::from(tok),
                Utc::now() + TimeDelta::hours(1),
            ))
        }
    };
    TokenSourceType::ExternalSource(Box::new(ExternalJwtFunctionSource::new(make)))
}

impl FirestoreClient {
    /// 프로파일 모드에 맞는 라이브 `FirestoreDb`를 생성한다.
    pub async fn connect(profile: &Profile, credential: Option<&Credential>) -> AppResult<Self> {
        let scopes: Vec<String> = FIRESTORE_SCOPES.iter().map(|s| s.to_string()).collect();
        let mut options = FirestoreDbOptions::new(profile.project_id.clone());

        let token_source = match profile.mode {
            ProfileMode::Emulator => {
                options = options.with_firebase_api_url(emulator_url(profile));
                static_token_source("owner".to_string())
            }
            ProfileMode::ServiceAccount => match credential {
                Some(Credential::ServiceAccount { json }) => {
                    TokenSourceType::Json(json.expose_secret().to_owned())
                }
                Some(_) => {
                    return Err(AppError::credential_invalid(
                        "stored credential kind does not match profile mode (service_account)",
                    ))
                }
                None => {
                    return Err(AppError::credential_not_found(
                        "service account profile has no stored credential",
                    ))
                }
            },
            ProfileMode::IdToken => match credential {
                Some(Credential::IdToken { token }) => {
                    static_token_source(token.expose_secret().to_owned())
                }
                Some(_) => {
                    return Err(AppError::credential_invalid(
                        "stored credential kind does not match profile mode (id_token)",
                    ))
                }
                None => {
                    return Err(AppError::credential_not_found(
                        "id_token profile has no stored credential",
                    ))
                }
            },
        };

        let db = FirestoreDb::with_options_token_source(options, scopes, token_source)
            .await
            .map_err(|_| AppError::Firestore {
                message: "failed to establish Firestore connection".into(),
            })?;

        Ok(Self {
            project_id: profile.project_id.clone(),
            mode: profile.mode,
            db,
        })
    }
}

/// 활성화 없이 연결 가능 여부만 검증하고 소요(ms)를 반환한다
/// (`test_profile` 도메인 로직, 원칙 4: 커맨드에서 분리).
pub async fn probe(profile: &Profile, credential: Option<Credential>) -> AppResult<u64> {
    let started = Instant::now();

    match profile.mode {
        ProfileMode::Emulator => {
            let url = emulator_url(profile);
            let hostport = url
                .trim_start_matches("http://")
                .trim_start_matches("https://");
            let addr = hostport
                .to_socket_addrs()
                .map_err(|_| AppError::Firestore {
                    message: format!("cannot resolve emulator host '{hostport}'"),
                })?
                .next()
                .ok_or_else(|| AppError::Firestore {
                    message: format!("no address for emulator host '{hostport}'"),
                })?;
            TcpStream::connect_timeout(&addr, Duration::from_secs(3)).map_err(|_| {
                AppError::Firestore {
                    message: format!("emulator unreachable at '{hostport}'"),
                }
            })?;
        }
        ProfileMode::ServiceAccount => match credential {
            Some(Credential::ServiceAccount { json }) => {
                ServiceAccountAuth::validate(&json).await?;
            }
            Some(Credential::IdToken { .. }) => {
                return Err(AppError::credential_invalid(
                    "stored credential kind does not match profile mode (service_account)",
                ));
            }
            None => {
                return Err(AppError::credential_not_found(
                    "service account profile has no stored credential",
                ));
            }
        },
        ProfileMode::IdToken => match credential {
            Some(Credential::IdToken { .. }) => {}
            Some(Credential::ServiceAccount { .. }) => {
                return Err(AppError::credential_invalid(
                    "stored credential kind does not match profile mode (id_token)",
                ));
            }
            None => {
                return Err(AppError::credential_not_found(
                    "id_token profile has no stored credential",
                ));
            }
        },
    }

    Ok(started.elapsed().as_millis() as u64)
}
