//! 클라이언트 후처리 검색 (`docs/04-query-dsl.md` "적용 규칙").
//!
//! 순수 도메인 — `tauri::*`/firestore 크레이트 무의존. Firestore가 표현하지
//! 못하는 검색(정규식/부분문자열/JSONPath)을 결과 디코드 직후 적용한다.
//! 비용이 있는 패턴은 [`compile`]에서 1회만 컴파일하고 문서마다 재사용한다
//! (원칙 8: 동시성은 적게·예측 가능하게).

use std::collections::BTreeMap;

use regex::{Regex, RegexBuilder};
use serde_json::{Map as JsonMap, Number, Value as Json};
use serde_json_path::JsonPath;

use crate::error::{AppError, AppResult};
use crate::query::dsl::{FirestoreValue, PostFilter};

/// 컴파일된 후처리 매처. `dsl.post_filter`가 있을 때만 생성된다.
pub struct Matcher {
    regex: Option<CompiledFields<Regex>>,
    contains: Option<CompiledContains>,
    jsonpath: Option<JsonPath>,
}

struct CompiledFields<P> {
    fields: Vec<String>,
    pattern: P,
}

struct CompiledContains {
    fields: Vec<String>,
    needle: String,
    case_insensitive: bool,
}

fn invalid(msg: impl Into<String>) -> AppError {
    AppError::InvalidQuery {
        message: msg.into(),
    }
}

/// `PostFilter` → `Matcher`. regex/jsonpath 컴파일 실패는
/// `AppError::InvalidQuery` (본문 미포함, 원칙 5).
pub fn compile(pf: &PostFilter) -> AppResult<Matcher> {
    let regex = match &pf.regex {
        Some(r) => {
            let pattern = RegexBuilder::new(&r.pattern)
                .case_insensitive(r.case_insensitive)
                .build()
                .map_err(|_| invalid("post_filter regex pattern does not compile"))?;
            Some(CompiledFields {
                fields: r.fields.clone(),
                pattern,
            })
        }
        None => None,
    };

    let contains = pf.contains.as_ref().map(|c| CompiledContains {
        fields: c.fields.clone(),
        needle: if c.case_insensitive {
            c.text.to_lowercase()
        } else {
            c.text.clone()
        },
        case_insensitive: c.case_insensitive,
    });

    let jsonpath = match &pf.jsonpath {
        Some(expr) => Some(
            JsonPath::parse(expr)
                .map_err(|_| invalid("post_filter jsonpath does not compile"))?,
        ),
        None => None,
    };

    Ok(Matcher {
        regex,
        contains,
        jsonpath,
    })
}

impl Matcher {
    /// 지정된 서브필터(regex/contains/jsonpath)를 **모두** 통과하면 true
    /// (AND). 미지정 서브필터는 무시. 셋 다 없으면 전부 통과.
    pub fn matches(&self, data: &BTreeMap<String, FirestoreValue>) -> bool {
        if let Some(rx) = &self.regex {
            let hit = rx.fields.iter().any(|f| {
                dotted(data, f)
                    .and_then(scalar_text)
                    .is_some_and(|t| rx.pattern.is_match(&t))
            });
            if !hit {
                return false;
            }
        }

        if let Some(c) = &self.contains {
            let hit = c.fields.iter().any(|f| {
                dotted(data, f).and_then(scalar_text).is_some_and(|t| {
                    if c.case_insensitive {
                        t.to_lowercase().contains(&c.needle)
                    } else {
                        t.contains(&c.needle)
                    }
                })
            });
            if !hit {
                return false;
            }
        }

        if let Some(jp) = &self.jsonpath {
            let json = data_to_json(data);
            if jp.query(&json).is_empty() {
                return false;
            }
        }

        true
    }
}

/// 점 표기 경로(`"profile.age"`)로 `map` 값을 따라 내려간다. 경로 없으면
/// `None` (에러 아님 — 비매칭으로 취급).
fn dotted<'a>(
    data: &'a BTreeMap<String, FirestoreValue>,
    path: &str,
) -> Option<&'a FirestoreValue> {
    let mut parts = path.split('.');
    let mut cur = data.get(parts.next()?)?;
    for p in parts {
        match cur {
            FirestoreValue::Map { value } => cur = value.get(p)?,
            _ => return None,
        }
    }
    Some(cur)
}

/// regex/contains 매칭 대상이 되는 스칼라 값의 문자열 표현.
/// 합성 타입(array/map/geo/null)은 `None`.
fn scalar_text(v: &FirestoreValue) -> Option<String> {
    match v {
        FirestoreValue::String { value }
        | FirestoreValue::Int { value }
        | FirestoreValue::Timestamp { value }
        | FirestoreValue::Reference { value }
        | FirestoreValue::Bytes { value } => Some(value.clone()),
        FirestoreValue::Double { value } => Some(value.to_string()),
        FirestoreValue::Bool { value } => Some(value.to_string()),
        FirestoreValue::Null
        | FirestoreValue::Geo { .. }
        | FirestoreValue::Array { .. }
        | FirestoreValue::Map { .. } => None,
    }
}

/// `FirestoreValue` → 자연 JSON 투영 (태그 유니온이 아닌 평문 값).
/// jsonpath 표현식이 직관적으로 동작하도록 한다.
fn to_plain_json(v: &FirestoreValue) -> Json {
    match v {
        FirestoreValue::Null => Json::Null,
        FirestoreValue::Bool { value } => Json::Bool(*value),
        FirestoreValue::Int { value } => value
            .parse::<i64>()
            .map(Json::from)
            .unwrap_or_else(|_| Json::String(value.clone())),
        FirestoreValue::Double { value } => {
            Number::from_f64(*value).map(Json::Number).unwrap_or(Json::Null)
        }
        FirestoreValue::String { value }
        | FirestoreValue::Bytes { value }
        | FirestoreValue::Timestamp { value }
        | FirestoreValue::Reference { value } => Json::String(value.clone()),
        FirestoreValue::Geo { lat, lng } => {
            let mut m = JsonMap::new();
            m.insert("lat".into(), json_num(*lat));
            m.insert("lng".into(), json_num(*lng));
            Json::Object(m)
        }
        FirestoreValue::Array { value } => {
            Json::Array(value.iter().map(to_plain_json).collect())
        }
        FirestoreValue::Map { value } => Json::Object(
            value
                .iter()
                .map(|(k, v)| (k.clone(), to_plain_json(v)))
                .collect(),
        ),
    }
}

fn json_num(f: f64) -> Json {
    Number::from_f64(f).map(Json::Number).unwrap_or(Json::Null)
}

fn data_to_json(data: &BTreeMap<String, FirestoreValue>) -> Json {
    Json::Object(
        data.iter()
            .map(|(k, v)| (k.clone(), to_plain_json(v)))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::dsl::{ContainsFilter, RegexFilter};

    fn s(v: &str) -> FirestoreValue {
        FirestoreValue::String { value: v.into() }
    }
    fn i(v: &str) -> FirestoreValue {
        FirestoreValue::Int { value: v.into() }
    }
    fn map(pairs: &[(&str, FirestoreValue)]) -> BTreeMap<String, FirestoreValue> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    fn doc() -> BTreeMap<String, FirestoreValue> {
        map(&[
            ("name", s("iPhone 15 Pro")),
            ("desc", s("flagship phone")),
            ("score", i("42")),
            (
                "profile",
                FirestoreValue::Map {
                    value: map(&[("age", i("30")), ("city", s("Seoul"))]),
                },
            ),
            (
                "tags",
                FirestoreValue::Array {
                    value: vec![s("urgent"), s("sale")],
                },
            ),
        ])
    }

    fn rf(fields: &[&str], pat: &str, ci: bool) -> PostFilter {
        PostFilter {
            regex: Some(RegexFilter {
                fields: fields.iter().map(|f| f.to_string()).collect(),
                pattern: pat.into(),
                case_insensitive: ci,
            }),
            contains: None,
            jsonpath: None,
        }
    }

    #[test]
    fn dotted_traverses_nested_map() {
        let d = doc();
        assert!(matches!(dotted(&d, "profile.city"), Some(FirestoreValue::String { value }) if value == "Seoul"));
        assert!(dotted(&d, "profile.missing").is_none());
        assert!(dotted(&d, "score.x").is_none());
    }

    #[test]
    fn scalar_text_only_for_scalars() {
        assert_eq!(scalar_text(&i("42")).as_deref(), Some("42"));
        assert_eq!(scalar_text(&s("hi")).as_deref(), Some("hi"));
        assert_eq!(
            scalar_text(&FirestoreValue::Bool { value: true }).as_deref(),
            Some("true")
        );
        assert!(scalar_text(&FirestoreValue::Null).is_none());
        assert!(scalar_text(&FirestoreValue::Array { value: vec![] }).is_none());
    }

    #[test]
    fn plain_json_is_natural_not_tagged() {
        let d = doc();
        let j = data_to_json(&d);
        assert_eq!(j["score"], serde_json::json!(42));
        assert_eq!(j["tags"], serde_json::json!(["urgent", "sale"]));
        assert_eq!(j["profile"]["city"], serde_json::json!("Seoul"));
    }

    #[test]
    fn regex_matches_any_field_or() {
        let m = compile(&rf(&["name", "desc"], r"flagship", false)).unwrap();
        assert!(m.matches(&doc()));
        let m = compile(&rf(&["name"], r"^nope$", false)).unwrap();
        assert!(!m.matches(&doc()));
    }

    #[test]
    fn regex_case_insensitive() {
        let m = compile(&rf(&["name"], r"iphone", true)).unwrap();
        assert!(m.matches(&doc()));
        let m = compile(&rf(&["name"], r"iphone", false)).unwrap();
        assert!(!m.matches(&doc()));
    }

    #[test]
    fn regex_on_int_field_uses_text_repr() {
        let m = compile(&rf(&["score"], r"^42$", false)).unwrap();
        assert!(m.matches(&doc()));
    }

    #[test]
    fn missing_field_is_non_match_not_error() {
        let m = compile(&rf(&["nonexistent"], r".*", false)).unwrap();
        assert!(!m.matches(&doc()));
    }

    #[test]
    fn contains_substring_with_case_option() {
        let pf = PostFilter {
            regex: None,
            contains: Some(ContainsFilter {
                fields: vec!["name".into()],
                text: "PRO".into(),
                case_insensitive: true,
            }),
            jsonpath: None,
        };
        assert!(compile(&pf).unwrap().matches(&doc()));
    }

    #[test]
    fn jsonpath_filter_on_array() {
        let pf = PostFilter {
            regex: None,
            contains: None,
            jsonpath: Some("$.tags[?@ == 'urgent']".into()),
        };
        assert!(compile(&pf).unwrap().matches(&doc()));
        let pf = PostFilter {
            regex: None,
            contains: None,
            jsonpath: Some("$.tags[?@ == 'missing']".into()),
        };
        assert!(!compile(&pf).unwrap().matches(&doc()));
    }

    #[test]
    fn subfilters_combine_with_and() {
        let pf = PostFilter {
            regex: Some(RegexFilter {
                fields: vec!["name".into()],
                pattern: "iPhone".into(),
                case_insensitive: false,
            }),
            contains: None,
            jsonpath: Some("$.tags[?@ == 'urgent']".into()),
        };
        assert!(compile(&pf).unwrap().matches(&doc()));
        // regex 통과, jsonpath 불통과 → 전체 불통과
        let pf = PostFilter {
            regex: Some(RegexFilter {
                fields: vec!["name".into()],
                pattern: "iPhone".into(),
                case_insensitive: false,
            }),
            contains: None,
            jsonpath: Some("$.tags[?@ == 'nope']".into()),
        };
        assert!(!compile(&pf).unwrap().matches(&doc()));
    }

    #[test]
    fn empty_post_filter_matches_all() {
        let m = compile(&PostFilter::default()).unwrap();
        assert!(m.matches(&doc()));
    }

    #[test]
    fn uncompilable_patterns_rejected() {
        assert!(compile(&rf(&["a"], "(", false)).is_err());
        let pf = PostFilter {
            regex: None,
            contains: None,
            jsonpath: Some("$[".into()),
        };
        assert!(compile(&pf).is_err());
    }
}
