//! 자격증명 형식 검증.
//!
//! 보안 규칙: 검증 실패 메시지에 **입력 본문 조각을 절대 넣지 않는다**.
//! serde_json 에러의 Display는 입력 일부를 포함할 수 있으므로 그대로
//! 전파하지 않고 일반 메시지로 치환한다.
//!
//! 호출 시점: `CredentialVault::set` 직전 (저장 전 1차 검증).
//! 서명 검증(서비스 계정 키의 실제 유효성, JWT 서명)은 여기서 하지 않는다 —
//! 그것은 프로파일 *활성화* 시점(Phase 1-B)의 책임이다.

use crate::error::{AppError, AppResult};
use crate::profile::model::Credential;

/// 자격증명 종류에 맞는 형식 검증으로 분기.
pub fn validate(cred: &Credential) -> AppResult<()> {
    use secrecy::ExposeSecret;
    match cred {
        Credential::ServiceAccount { json } => validate_service_account_json(json.expose_secret()),
        Credential::IdToken { token } => validate_id_token_format(token.expose_secret()),
    }
}

/// ID 토큰의 **형식**만 검증 (서명 검증 아님).
///
/// JWT는 `header.payload.signature` 3개 세그먼트가 `.`로 구분되며 각
/// 세그먼트는 base64url. 여기서는 구조만 본다 — 실제 서명/만료 검증은
/// 활성화 시 `jsonwebtoken`으로 수행한다.
fn validate_id_token_format(token: &str) -> AppResult<()> {
    let token = token.trim();
    let segments: Vec<&str> = token.split('.').collect();
    let structurally_ok = segments.len() == 3
        && segments.iter().all(|s| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        });

    if structurally_ok {
        Ok(())
    } else {
        // 토큰 본문은 메시지에 넣지 않는다.
        Err(AppError::credential_invalid(
            "ID token is not a well-formed JWT",
        ))
    }
}

/// 서비스 계정 JSON의 필수 필드 검증.
///
/// `raw`는 사용자가 붙여넣은 JSON 문자열 전체다. serde_json으로 파싱한 뒤
/// 필수 필드 존재를 확인한다.
///
/// 정책: **구조 확인**. gcp_auth가 액세스 토큰을 발급하는 데 필요한
/// 최소 재료(`project_id`, `private_key`, `client_email`)가 비어있지 않은
/// 문자열로 존재하는지만 본다. 키의 *암호학적* 유효성(서명 가능 여부)은
/// 활성화 시점에 gcp_auth가 판정한다. OAuth client JSON 등 잘못 붙여넣은
/// 파일은 이 필드들이 없으므로 자연히 걸러진다.
fn validate_service_account_json(raw: &str) -> AppResult<()> {
    // JSON 파싱. serde_json 에러 Display는 입력 일부를 포함할 수 있으므로
    // 절대 그대로 쓰지 말고 일반 메시지로 치환한다.
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|_| AppError::credential_invalid("service account is not valid JSON"))?;

    // 필드 *이름*은 비밀이 아니므로 메시지에 넣어도 안전하다. 필드 *값*과
    // `raw` 조각은 절대 넣지 않는다.
    for key in ["project_id", "private_key", "client_email"] {
        let present_non_empty = value
            .get(key)
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.trim().is_empty());
        if !present_non_empty {
            return Err(AppError::credential_invalid(format!(
                "service account JSON missing required field '{key}'"
            )));
        }
    }

    Ok(())
}
