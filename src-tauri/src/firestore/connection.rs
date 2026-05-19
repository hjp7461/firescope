//! 모드별 Firestore 연결 설정 해석.
//!
//! 여기서는 "어디에·어떤 방식으로 붙을지"를 확정만 한다. 실제
//! [`firestore::FirestoreDb`] gRPC 채널은 첫 쿼리 시점(Phase 2)에
//! 이 설정을 입력으로 생성된다. service_account 자격증명의 실제 유효성은
//! 활성화 시점에 `auth::ServiceAccountAuth`가 토큰 왕복으로 검증하므로,
//! 라이브 클라이언트가 없어도 세션 활성화는 의미 있게 검증된다.

use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::auth::ServiceAccountAuth;
use crate::error::{AppError, AppResult};
use crate::profile::{Credential, Profile, ProfileMode};

/// 기본 에뮬레이터 호스트 (`docs/02-architecture.md` 호스트 우선순위 3순위).
const DEFAULT_EMULATOR_HOST: &str = "localhost:8080";
const EMULATOR_HOST_ENV: &str = "FIRESTORE_EMULATOR_HOST";

/// 활성 세션이 보유하는 Firestore 접속 설정.
///
/// Phase 2가 이 값으로 `FirestoreDb`를 만든다. 자격증명 본문은 담지
/// 않는다 — service_account/id_token 비밀은 `auth::AuthHandle`이 보유한다.
#[derive(Debug, Clone)]
pub struct FirestoreClient {
    pub project_id: String,
    pub mode: ProfileMode,
    /// 에뮬레이터 모드일 때만 `Some`. `http://host:port`로 정규화됨.
    pub emulator_url: Option<String>,
}

impl FirestoreClient {
    /// 프로파일 모드에 맞는 연결 설정을 해석한다.
    ///
    /// 에뮬레이터 호스트 우선순위 (`docs/02-architecture.md`):
    /// 1. 프로파일의 `firestore_host`
    /// 2. 환경변수 `FIRESTORE_EMULATOR_HOST`
    /// 3. 기본값 `localhost:8080`
    pub fn connect(profile: &Profile) -> AppResult<Self> {
        let emulator_url = match profile.mode {
            ProfileMode::Emulator => {
                let host = profile
                    .firestore_host
                    .clone()
                    .or_else(|| std::env::var(EMULATOR_HOST_ENV).ok())
                    .unwrap_or_else(|| DEFAULT_EMULATOR_HOST.to_string());
                Some(ensure_url_scheme(&host))
            }
            ProfileMode::ServiceAccount | ProfileMode::IdToken => None,
        };

        Ok(Self {
            project_id: profile.project_id.clone(),
            mode: profile.mode,
            emulator_url,
        })
    }
}

/// 활성화 없이 연결 가능 여부만 검증하고 소요(ms)를 반환한다
/// (`test_profile` 도메인 로직, 원칙 4: 커맨드에서 분리).
///
/// - emulator: 호스트 TCP 도달성
/// - service_account: 실제 토큰 왕복 (`ServiceAccountAuth::validate`)
/// - id_token: 자격증명 존재/종류 일치 확인 (서명 검증은 후속 단계)
pub async fn probe(profile: &Profile, credential: Option<Credential>) -> AppResult<u64> {
    let started = Instant::now();

    match profile.mode {
        ProfileMode::Emulator => {
            let client = FirestoreClient::connect(profile)?;
            let url = client.emulator_url.unwrap_or_default();
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

/// 스킴이 없으면 `http://`를 붙인다 (에뮬레이터는 평문 gRPC).
fn ensure_url_scheme(host: &str) -> String {
    if host.contains("://") {
        host.to_string()
    } else {
        format!("http://{host}")
    }
}
