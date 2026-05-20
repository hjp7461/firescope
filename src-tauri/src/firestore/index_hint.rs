//! Firestore 누락 인덱스 에러에서 콘솔 URL 추출 (Phase 8-A).
//!
//! Firestore는 합성 인덱스가 필요한 쿼리를 거부할 때 에러 메시지에
//! `https://console.firebase.google.com/.../firestore/indexes?create_composite=...`
//! 형태의 URL을 포함시킨다. 사용자가 클릭 한 번으로 인덱스를 생성할 수
//! 있도록 그 URL만 추출해 `query:error` 페이로드의 별도 필드로 전달한다.
//!
//! 메시지 본문은 일반화된 형태(`"error while streaming query results"`)로
//! 유지되어 자격증명·내부 상세가 새지 않는다.

use once_cell::sync::Lazy;
use regex::Regex;

/// console.firebase.google.com 또는 console.cloud.google.com 의 인덱스 URL.
///
/// Firestore SDK가 보내는 메시지는 따옴표/괄호로 둘러싸여 있을 수 있으므로
/// 공백/따옴표/괄호를 stop char로 본다.
static INDEX_URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"https://console\.(?:firebase|cloud)\.google\.com/[^\s'"\)\]>]+"#,
    )
    .expect("static regex must compile")
});

/// 에러 메시지에서 첫 번째 Firestore 콘솔 URL을 반환. 없으면 None.
pub fn extract_firestore_index_url(message: &str) -> Option<String> {
    INDEX_URL_RE
        .find(message)
        .map(|m| trim_trailing_punct(m.as_str()).to_string())
}

/// URL 끝의 흔한 문장부호(`.,;`)는 보통 URL의 일부가 아니라 메시지 종결이므로 제거.
fn trim_trailing_punct(s: &str) -> &str {
    s.trim_end_matches(|c: char| matches!(c, '.' | ',' | ';'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_firebase_console_create_composite_url() {
        let msg = r#"failed_precondition: The query requires an index. You can create it here: https://console.firebase.google.com/v1/r/project/demo/firestore/indexes?create_composite=ClRwcm9q"#;
        let url = extract_firestore_index_url(msg).expect("url");
        assert!(url.starts_with("https://console.firebase.google.com/"));
        assert!(url.contains("create_composite="));
    }

    #[test]
    fn extracts_cloud_console_variant() {
        let msg = "Index required. See https://console.cloud.google.com/datastore/indexes?project=demo for details.";
        let url = extract_firestore_index_url(msg).expect("url");
        assert_eq!(
            url,
            "https://console.cloud.google.com/datastore/indexes?project=demo"
        );
    }

    #[test]
    fn trims_trailing_punctuation() {
        // 문장 끝의 . , ; 는 URL이 아니다.
        let msg = "...visit https://console.firebase.google.com/x/y.";
        let url = extract_firestore_index_url(msg).expect("url");
        assert!(!url.ends_with('.'));
    }

    #[test]
    fn returns_none_when_no_url_present() {
        assert_eq!(
            extract_firestore_index_url("plain firestore error, no url"),
            None
        );
    }

    #[test]
    fn returns_none_for_unrelated_urls() {
        // GitHub URL은 인덱스 콘솔이 아니다.
        assert_eq!(
            extract_firestore_index_url("see https://github.com/foo/bar/issues/1"),
            None
        );
    }

    #[test]
    fn handles_url_inside_quotes() {
        let msg = r#"detail: "https://console.firebase.google.com/v1/r/project/p/firestore/indexes?create_composite=abc""#;
        let url = extract_firestore_index_url(msg).expect("url");
        assert!(!url.ends_with('"'));
        assert!(url.contains("create_composite=abc"));
    }
}
