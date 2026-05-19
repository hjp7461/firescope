//! 인증 계층.
//!
//! 프로파일 모드별로 Firestore 호출에 쓸 Bearer 토큰을 공급한다
//! (`docs/02-architecture.md` "인증 모델").
//!
//! | 모드            | 구현                  | 토큰        | 갱신            |
//! |-----------------|-----------------------|-------------|-----------------|
//! | Emulator        | [`EmulatorAuth`]      | 없음(None)  | 없음            |
//! | ServiceAccount  | [`ServiceAccountAuth`]| gcp_auth    | 만료 5분 전 자동 |
//! | IdToken         | [`IdTokenAuth`]       | 입력값 그대로 | 없음(재입력)    |

pub mod emulator;
pub mod id_token;
pub mod service_account;

use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use secrecy::SecretString;

use crate::error::AppResult;
use crate::profile::ProfileMode;

pub use emulator::EmulatorAuth;
pub use id_token::IdTokenAuth;
pub use service_account::ServiceAccountAuth;

/// Firestore 액세스에 필요한 OAuth 스코프.
pub const FIRESTORE_SCOPES: &[&str] = &["https://www.googleapis.com/auth/datastore"];

/// 활성 세션이 보유하는 인증 핸들.
///
/// `async fn`을 트레잇에 직접 두면 `dyn` 객체로 만들 수 없으므로,
/// 이미 의존 중인 `futures`의 [`BoxFuture`]를 반환해 dyn-호환을 유지한다
/// (`async-trait` 의존성 추가를 피한다). 세션은 이를 `Box<dyn AuthHandle>`로
/// 보관한다.
pub trait AuthHandle: Send + Sync {
    /// Authorization 헤더에 넣을 Bearer 토큰. 에뮬레이터는 `Ok(None)`.
    ///
    /// 반환된 `SecretString`은 절대 로그/에러/IPC로 새지 않아야 한다.
    fn bearer_token(&self) -> BoxFuture<'_, AppResult<Option<SecretString>>>;

    /// 현재 토큰 만료 시각 (`Session.expires_at`용). 없으면 `None`.
    fn expires_at(&self) -> Option<DateTime<Utc>>;

    fn mode(&self) -> ProfileMode;

    /// 토큰 강제 갱신 후 새 만료 시각 반환.
    ///
    /// 기본 구현은 갱신할 토큰이 없는 모드(emulator/id_token)용으로,
    /// 현재 만료 시각을 그대로 돌려준다(무동작). 서비스 계정은 실제
    /// 재발급하도록 오버라이드한다.
    fn force_refresh(&self) -> BoxFuture<'_, AppResult<Option<DateTime<Utc>>>> {
        let current = self.expires_at();
        Box::pin(async move { Ok(current) })
    }
}
