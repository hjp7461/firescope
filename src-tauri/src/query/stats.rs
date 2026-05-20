//! 컬렉션 통계 (`docs/03-ipc-contract.md` §5 `compute_stats`).
//!
//! 순수 도메인 — `tauri::*`/firestore 크레이트 무의존. 활성 쿼리 결과의
//! top-level 필드에 대해 타입 분포·null/missing 비율·상위 샘플 값을
//! 1-pass로 산출한다 (원칙 5 — 운영 데이터를 메모리에 길게 유지하지 않음).
//!
//! nested 필드 진입은 의도적으로 하지 않는다. 큰 본문이 IPC 응답에 실리는
//! 것을 막고, 사용자가 통계 화면에서 의도치 않게 PII 본문을 보지 않게
//! 하기 위함. nested 지원은 백로그.

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;

use crate::firestore::Document;
use crate::query::dsl::FirestoreValue;

/// IPC `compute_stats`의 `top_samples` 허용 상한 (응답 크기 보호).
pub const MAX_TOP_SAMPLES: usize = 50;
/// 단일 샘플 값 문자열의 최대 글자 수. 초과분은 `…`로 잘라낸다.
const SAMPLE_TEXT_MAX_CHARS: usize = 120;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StatsReport {
    pub sample_size: u64,
    /// `"matched"` | `"scanned"` — 호출자가 그대로 패스스루.
    pub source: String,
    pub fields: Vec<FieldStat>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FieldStat {
    pub key: String,
    pub present: u64,
    pub missing: u64,
    pub null_count: u64,
    pub types: Vec<TypeBucket>,
    pub samples: Vec<SampleValue>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TypeBucket {
    #[serde(rename = "type")]
    pub type_name: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SampleValue {
    pub value: String,
    pub count: u64,
}

/// `top_samples`를 `[0, MAX_TOP_SAMPLES]` 범위로 클램프한다.
pub fn clamp_top_samples(n: usize) -> usize {
    n.min(MAX_TOP_SAMPLES)
}

#[derive(Default)]
struct FieldAcc {
    present: u64,
    null_count: u64,
    type_counts: HashMap<&'static str, u64>,
    sample_counts: HashMap<String, u64>,
}

/// `docs`를 1-pass로 소비하며 필드별 통계를 산출한다.
///
/// - `source`: `"matched"` | `"scanned"` — IPC에서 그대로 패스스루.
/// - `top_samples`: 필드별 상위 샘플 값 개수. caller가 [`clamp_top_samples`]로
///   클램프한다.
/// - 반환 `fields`는 키 알파벳순. 각 필드의 `types`/`samples`는 count 내림차순,
///   동률은 이름/값 알파벳순으로 안정 정렬된다.
pub fn compute_field_stats<I>(docs: I, source: &str, top_samples: usize) -> StatsReport
where
    I: IntoIterator<Item = Document>,
{
    let mut sample_size: u64 = 0;
    let mut accs: HashMap<String, FieldAcc> = HashMap::new();
    let mut all_keys: BTreeSet<String> = BTreeSet::new();

    for doc in docs {
        sample_size += 1;
        for (k, v) in &doc.data {
            if !all_keys.contains(k) {
                all_keys.insert(k.clone());
            }
            let entry = accs.entry(k.clone()).or_default();
            entry.present += 1;
            let t = type_name(v);
            *entry.type_counts.entry(t).or_default() += 1;
            if matches!(v, FirestoreValue::Null) {
                entry.null_count += 1;
            }
            let sample = stringify_sample(v);
            *entry.sample_counts.entry(sample).or_default() += 1;
        }
    }

    let fields: Vec<FieldStat> = all_keys
        .into_iter()
        .map(|key| {
            let acc = accs.remove(&key).unwrap_or_default();

            let mut types: Vec<TypeBucket> = acc
                .type_counts
                .into_iter()
                .map(|(t, c)| TypeBucket {
                    type_name: t.to_string(),
                    count: c,
                })
                .collect();
            types.sort_by(|a, b| {
                b.count
                    .cmp(&a.count)
                    .then_with(|| a.type_name.cmp(&b.type_name))
            });

            let mut samples: Vec<SampleValue> = acc
                .sample_counts
                .into_iter()
                .map(|(v, c)| SampleValue { value: v, count: c })
                .collect();
            samples.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.value.cmp(&b.value)));
            samples.truncate(top_samples);

            FieldStat {
                key,
                present: acc.present,
                missing: sample_size.saturating_sub(acc.present),
                null_count: acc.null_count,
                types,
                samples,
            }
        })
        .collect();

    StatsReport {
        sample_size,
        source: source.to_string(),
        fields,
    }
}

fn type_name(v: &FirestoreValue) -> &'static str {
    match v {
        FirestoreValue::Null => "null",
        FirestoreValue::Bool { .. } => "bool",
        FirestoreValue::Int { .. } => "int",
        FirestoreValue::Double { .. } => "double",
        FirestoreValue::String { .. } => "string",
        FirestoreValue::Bytes { .. } => "bytes",
        FirestoreValue::Timestamp { .. } => "timestamp",
        FirestoreValue::Reference { .. } => "reference",
        FirestoreValue::Geo { .. } => "geo",
        FirestoreValue::Array { .. } => "array",
        FirestoreValue::Map { .. } => "map",
    }
}

/// `FirestoreValue` → 짧은 사람 읽기 좋은 문자열.
///
/// nested(array/map)은 길이 요약만 노출한다 (`Array(n)`, `Map(n)`).
/// 본문을 넣지 않는 이유: ① IPC 응답 크기 보호, ② 통계 화면에서 의도치 않게
/// PII가 노출되는 경로 차단. `SAMPLE_TEXT_MAX_CHARS`를 넘는 문자열은
/// 끝에 `…`를 붙여 잘라낸다.
fn stringify_sample(v: &FirestoreValue) -> String {
    let raw = match v {
        FirestoreValue::Null => "null".to_string(),
        FirestoreValue::Bool { value } => value.to_string(),
        FirestoreValue::Int { value } => value.clone(),
        FirestoreValue::Double { value } => format!("{value}"),
        FirestoreValue::String { value } => value.clone(),
        FirestoreValue::Bytes { .. } => "<bytes>".to_string(),
        FirestoreValue::Timestamp { value } => value.clone(),
        FirestoreValue::Reference { value } => value.clone(),
        FirestoreValue::Geo { lat, lng } => format!("{lat},{lng}"),
        FirestoreValue::Array { value } => format!("Array({})", value.len()),
        FirestoreValue::Map { value } => format!("Map({})", value.len()),
    };
    truncate_chars(&raw, SAMPLE_TEXT_MAX_CHARS)
}

fn truncate_chars(s: &str, max: usize) -> String {
    for (count, (idx, _)) in s.char_indices().enumerate() {
        if count == max {
            let mut out = String::with_capacity(idx + 3);
            out.push_str(&s[..idx]);
            out.push('…');
            return out;
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn doc(id: &str, fields: &[(&str, FirestoreValue)]) -> Document {
        let mut data = BTreeMap::new();
        for (k, v) in fields {
            data.insert((*k).to_string(), v.clone());
        }
        Document {
            path: format!("users/{id}"),
            id: id.into(),
            parent: "users".into(),
            data,
            create_time: None,
            update_time: None,
        }
    }

    fn s(v: &str) -> FirestoreValue {
        FirestoreValue::String { value: v.into() }
    }
    fn i(n: i64) -> FirestoreValue {
        FirestoreValue::Int {
            value: n.to_string(),
        }
    }

    #[test]
    fn empty_input_yields_zero_sample() {
        let rep = compute_field_stats(Vec::<Document>::new(), "matched", 5);
        assert_eq!(rep.sample_size, 0);
        assert_eq!(rep.source, "matched");
        assert!(rep.fields.is_empty());
    }

    #[test]
    fn single_doc_single_field_int() {
        let rep = compute_field_stats(vec![doc("a", &[("n", i(42))])], "matched", 5);
        assert_eq!(rep.sample_size, 1);
        assert_eq!(rep.fields.len(), 1);
        let f = &rep.fields[0];
        assert_eq!(f.key, "n");
        assert_eq!(f.present, 1);
        assert_eq!(f.missing, 0);
        assert_eq!(f.null_count, 0);
        assert_eq!(
            f.types,
            vec![TypeBucket {
                type_name: "int".into(),
                count: 1
            }]
        );
        assert_eq!(
            f.samples,
            vec![SampleValue {
                value: "42".into(),
                count: 1
            }]
        );
    }

    #[test]
    fn missing_counts_field_absence_not_null() {
        // doc1: n=1, doc2: n=null, doc3: 필드 자체 없음
        let rep = compute_field_stats(
            vec![
                doc("a", &[("n", i(1))]),
                doc("b", &[("n", FirestoreValue::Null)]),
                doc("c", &[]),
            ],
            "matched",
            5,
        );
        let f = rep.fields.iter().find(|f| f.key == "n").unwrap();
        assert_eq!(rep.sample_size, 3);
        assert_eq!(f.present, 2, "null 값도 present로 친다");
        assert_eq!(f.missing, 1, "필드 자체가 없는 doc만 missing");
        assert_eq!(f.null_count, 1, "FirestoreValue::Null만 null_count");
    }

    #[test]
    fn types_sorted_desc_with_alpha_tiebreak() {
        let rep = compute_field_stats(
            vec![
                doc("a", &[("v", s("x"))]),
                doc("b", &[("v", s("y"))]),
                doc("c", &[("v", i(1))]),
                doc("d", &[("v", FirestoreValue::Bool { value: true })]),
            ],
            "matched",
            5,
        );
        let f = &rep.fields[0];
        // string=2, int=1, bool=1 → bool 우선(알파벳)
        assert_eq!(f.types[0].type_name, "string");
        assert_eq!(f.types[0].count, 2);
        assert_eq!(f.types[1].type_name, "bool");
        assert_eq!(f.types[2].type_name, "int");
    }

    #[test]
    fn samples_truncated_to_top_n_and_sorted() {
        let rep = compute_field_stats(
            vec![
                doc("a", &[("tag", s("alpha"))]),
                doc("b", &[("tag", s("alpha"))]),
                doc("c", &[("tag", s("alpha"))]),
                doc("d", &[("tag", s("beta"))]),
                doc("e", &[("tag", s("beta"))]),
                doc("f", &[("tag", s("gamma"))]),
                doc("g", &[("tag", s("delta"))]),
            ],
            "matched",
            2,
        );
        let f = &rep.fields[0];
        assert_eq!(f.samples.len(), 2, "top_samples=2");
        assert_eq!(f.samples[0].value, "alpha");
        assert_eq!(f.samples[0].count, 3);
        assert_eq!(f.samples[1].value, "beta");
        assert_eq!(f.samples[1].count, 2);
    }

    #[test]
    fn top_samples_zero_yields_empty_samples() {
        let rep = compute_field_stats(vec![doc("a", &[("n", i(1))])], "matched", 0);
        assert!(rep.fields[0].samples.is_empty());
    }

    #[test]
    fn nested_array_map_only_show_length_not_body() {
        let arr = FirestoreValue::Array {
            value: vec![s("secret"), s("body")],
        };
        let mut m = BTreeMap::new();
        m.insert("token".to_string(), s("eyJhbGciOiJIUzI1NiJ9..."));
        let map = FirestoreValue::Map { value: m };

        let rep = compute_field_stats(
            vec![
                doc("a", &[("arr", arr)]),
                doc("b", &[("obj", map)]),
            ],
            "matched",
            5,
        );

        let arr_field = rep.fields.iter().find(|f| f.key == "arr").unwrap();
        assert_eq!(arr_field.samples[0].value, "Array(2)");
        assert!(
            !arr_field.samples[0].value.contains("secret"),
            "nested 본문이 노출되면 안 된다"
        );

        let obj_field = rep.fields.iter().find(|f| f.key == "obj").unwrap();
        assert_eq!(obj_field.samples[0].value, "Map(1)");
        assert!(
            !obj_field.samples[0].value.contains("eyJhbGciOiJ"),
            "nested 본문이 노출되면 안 된다"
        );
    }

    #[test]
    fn long_string_sample_truncated_with_ellipsis() {
        let long: String = "가".repeat(200);
        let rep = compute_field_stats(vec![doc("a", &[("k", s(&long))])], "matched", 5);
        let sample = &rep.fields[0].samples[0].value;
        let char_count = sample.chars().count();
        assert!(
            char_count <= SAMPLE_TEXT_MAX_CHARS + 1,
            "최대 글자 + ellipsis 한 글자"
        );
        assert!(sample.ends_with('…'));
    }

    #[test]
    fn fields_sorted_alphabetically() {
        let rep = compute_field_stats(
            vec![
                doc("a", &[("zeta", i(1)), ("alpha", i(2)), ("mike", i(3))]),
            ],
            "matched",
            5,
        );
        let keys: Vec<_> = rep.fields.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, vec!["alpha", "mike", "zeta"]);
    }

    #[test]
    fn clamp_top_samples_respects_max() {
        assert_eq!(clamp_top_samples(0), 0);
        assert_eq!(clamp_top_samples(5), 5);
        assert_eq!(clamp_top_samples(MAX_TOP_SAMPLES), MAX_TOP_SAMPLES);
        assert_eq!(clamp_top_samples(MAX_TOP_SAMPLES + 1), MAX_TOP_SAMPLES);
        assert_eq!(clamp_top_samples(usize::MAX), MAX_TOP_SAMPLES);
    }

    #[test]
    fn source_is_passed_through() {
        let rep = compute_field_stats(Vec::<Document>::new(), "scanned", 5);
        assert_eq!(rep.source, "scanned");
    }
}
