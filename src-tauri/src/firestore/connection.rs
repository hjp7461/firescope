//! 모드별 Firestore 연결 설정 해석.
//!
//! 여기서는 "어디에·어떤 방식으로 붙을지"를 확정만 한다. 실제
//! [`firestore::FirestoreDb`] gRPC 채널은 첫 쿼리 시점(Phase 2)에
//! 이 설정을 입력으로 생성된다. service_account 자격증명의 실제 유효성은
//! 활성화 시점에 `auth::ServiceAccountAuth`가 토큰 왕복으로 검증하므로,
//! 라이브 클라이언트가 없어도 세션 활성화는 의미 있게 검증된다.

use crate::error::AppResult;
use crate::profile::{Profile, ProfileMode};

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

/// 스킴이 없으면 `http://`를 붙인다 (에뮬레이터는 평문 gRPC).
fn ensure_url_scheme(host: &str) -> String {
    if host.contains("://") {
        host.to_string()
    } else {
        format!("http://{host}")
    }
}
