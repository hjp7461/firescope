//! 컬렉션 통계 (`docs/03-ipc-contract.md` §5 `compute_stats`).
//!
//! 순수 도메인 — `tauri::*`/firestore 크레이트 무의존. 활성 쿼리 결과의
//! 필드에 대해 타입 분포·null/missing 비율·상위 샘플 값을 1-pass로 산출한다
//! (원칙 5 — 운영 데이터를 메모리에 길게 유지하지 않음).
//!
//! Phase 10에서 nested map 펼침을 옵션으로 도입했다. 배열은 여전히 펼치지
//! 않는다 — 본문이 응답에 실리는 것과 PII 노출을 막기 위해.

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;

use crate::firestore::Document;
use crate::query::dsl::FirestoreValue;

/// IPC `compute_stats`의 `top_samples` 허용 상한 (응답 크기 보호).
pub const MAX_TOP_SAMPLES: usize = 50;
/// nested 펼침의 최대 깊이 상한. 너무 깊으면 dot-path가 폭발하므로 5로 제한.
pub const MAX_NESTED_DEPTH: usize = 5;
/// `max_depth`를 명시하지 않았을 때 사용하는 기본 깊이.
pub const DEFAULT_NESTED_DEPTH: usize = 3;
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
    /// 필드 경로. top-level이면 단일 키, nested면 dot-path (`profile.email`).
    pub key: String,
    /// 0 = top-level, 1 = nested 1단계, … (Phase 10).
    pub depth: u32,
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

/// `max_depth`를 `[1, MAX_NESTED_DEPTH]` 범위로 클램프한다.
/// `0` 또는 그 아래는 1로 올린다 — `include_nested=true`인데 깊이 0이면 의도가
/// 모호하므로 최소 1단계는 펼쳐 준다.
pub fn clamp_max_depth(n: usize) -> usize {
    n.clamp(1, MAX_NESTED_DEPTH)
}

#[derive(Default)]
struct FieldAcc {
    depth: u32,
    present: u64,
    null_count: u64,
    type_counts: HashMap<&'static str, u64>,
    sample_counts: HashMap<String, u64>,
}

/// `docs`를 1-pass로 소비하며 **top-level** 필드 통계를 산출한다 (v0.7 동작).
/// nested 펼침을 원하면 [`compute_field_stats_nested`]를 사용한다.
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
    compute_field_stats_nested(docs, source, top_samples, false, DEFAULT_NESTED_DEPTH)
}

/// `compute_field_stats`의 nested 확장 (Phase 10).
///
/// `include_nested = false`이면 [`compute_field_stats`]와 동일 동작 (후방호환).
/// `include_nested = true`이면 map만 dot-path로 재귀해 자식 키도 별도 row로
/// 산출한다. 배열은 본문 노출/응답 폭발을 막기 위해 펼치지 않는다.
///
/// `max_depth`는 [`clamp_max_depth`]로 `1..=MAX_NESTED_DEPTH` 범위로 클램프해
/// 호출하는 것이 권장된다. depth=1은 top-level 한 단계 아래까지 (`a.b`는
/// 포함, `a.b.c`는 제외).
pub fn compute_field_stats_nested<I>(
    docs: I,
    source: &str,
    top_samples: usize,
    include_nested: bool,
    max_depth: usize,
) -> StatsReport
where
    I: IntoIterator<Item = Document>,
{
    let mut sample_size: u64 = 0;
    let mut accs: HashMap<String, FieldAcc> = HashMap::new();
    let mut all_keys: BTreeSet<String> = BTreeSet::new();

    let effective_depth = if include_nested {
        clamp_max_depth(max_depth)
    } else {
        0 // top-level만
    };

    for doc in docs {
        sample_size += 1;
        for (k, v) in &doc.data {
            visit_field(k.as_str(), v, 0, effective_depth, &mut accs, &mut all_keys);
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
                depth: acc.depth,
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

/// 단일 필드를 누적기에 기록하고, map이면 깊이 한도 내에서 재귀로 자식 키도
/// 별도 row로 기록한다. 배열은 의도적으로 펼치지 않는다 (본문 보호).
fn visit_field(
    path: &str,
    value: &FirestoreValue,
    depth: u32,
    max_depth: usize,
    accs: &mut HashMap<String, FieldAcc>,
    all_keys: &mut BTreeSet<String>,
) {
    if !all_keys.contains(path) {
        all_keys.insert(path.to_string());
    }
    let entry = accs.entry(path.to_string()).or_default();
    entry.depth = depth;
    entry.present += 1;
    let t = type_name(value);
    *entry.type_counts.entry(t).or_default() += 1;
    if matches!(value, FirestoreValue::Null) {
        entry.null_count += 1;
    }
    let sample = stringify_sample(value);
    *entry.sample_counts.entry(sample).or_default() += 1;

    if let FirestoreValue::Map { value: children } = value {
        if (depth as usize) < max_depth {
            for (child_key, child_value) in children {
                let child_path = format!("{path}.{child_key}");
                visit_field(
                    &child_path,
                    child_value,
                    depth + 1,
                    max_depth,
                    accs,
                    all_keys,
                );
            }
        }
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

    // --- Phase 10 nested 펼침 ---

    fn map(entries: &[(&str, FirestoreValue)]) -> FirestoreValue {
        let mut m = BTreeMap::new();
        for (k, v) in entries {
            m.insert((*k).to_string(), v.clone());
        }
        FirestoreValue::Map { value: m }
    }

    #[test]
    fn nested_disabled_keeps_v07_behavior() {
        // include_nested=false → top-level만 (depth=0), 기존 동작 후방호환.
        let rep = compute_field_stats_nested(
            vec![doc("a", &[("profile", map(&[("email", s("x@y.com"))]))])],
            "matched",
            5,
            false,
            3,
        );
        assert_eq!(rep.fields.len(), 1);
        assert_eq!(rep.fields[0].key, "profile");
        assert_eq!(rep.fields[0].depth, 0);
        // 본문은 여전히 노출 안 됨 — Map(1) 요약만.
        assert_eq!(rep.fields[0].samples[0].value, "Map(1)");
    }

    #[test]
    fn nested_enabled_flattens_map_with_dot_path() {
        // include_nested=true → map은 자체 row + 자식 키 row.
        let rep = compute_field_stats_nested(
            vec![doc("a", &[("profile", map(&[("email", s("x@y.com"))]))])],
            "matched",
            5,
            true,
            3,
        );
        let keys: Vec<_> = rep.fields.iter().map(|f| f.key.as_str()).collect();
        assert!(keys.contains(&"profile"), "부모 map row 유지");
        assert!(
            keys.contains(&"profile.email"),
            "자식 키가 dot-path로 펼쳐져야 한다"
        );
        let parent = rep.fields.iter().find(|f| f.key == "profile").unwrap();
        assert_eq!(parent.depth, 0);
        let child = rep
            .fields
            .iter()
            .find(|f| f.key == "profile.email")
            .unwrap();
        assert_eq!(child.depth, 1);
        assert_eq!(child.types[0].type_name, "string");
        assert_eq!(child.samples[0].value, "x@y.com");
    }

    #[test]
    fn nested_respects_max_depth() {
        // depth 3까지 펼침: a.b.c.d (depth=3)는 포함, a.b.c.d.e는 제외.
        let level5 = map(&[("e", s("leaf"))]);
        let level4 = map(&[("d", level5)]);
        let level3 = map(&[("c", level4)]);
        let level2 = map(&[("b", level3)]);
        let rep = compute_field_stats_nested(
            vec![doc("a", &[("a", level2)])],
            "matched",
            5,
            true,
            3,
        );
        let keys: Vec<_> = rep.fields.iter().map(|f| f.key.as_str()).collect();
        assert!(keys.contains(&"a")); // depth 0
        assert!(keys.contains(&"a.b")); // depth 1
        assert!(keys.contains(&"a.b.c")); // depth 2
        assert!(keys.contains(&"a.b.c.d")); // depth 3 (마지막 허용)
        assert!(
            !keys.contains(&"a.b.c.d.e"),
            "max_depth=3은 leaf의 자식까지는 펼치지 않는다"
        );
    }

    #[test]
    fn nested_max_depth_one_flattens_one_level_only() {
        let rep = compute_field_stats_nested(
            vec![doc(
                "a",
                &[("p", map(&[("q", map(&[("r", s("leaf"))]))]))],
            )],
            "matched",
            5,
            true,
            1,
        );
        let keys: Vec<_> = rep.fields.iter().map(|f| f.key.as_str()).collect();
        assert!(keys.contains(&"p"));
        assert!(keys.contains(&"p.q"));
        assert!(!keys.contains(&"p.q.r"), "max_depth=1은 1단계만 펼친다");
    }

    #[test]
    fn nested_arrays_are_not_flattened_even_at_top_level() {
        // top-level array도 펼치지 않고 Array(n) 요약만 — 본문 노출 차단.
        let arr_of_maps = FirestoreValue::Array {
            value: vec![map(&[("secret", s("leak-me"))])],
        };
        let rep = compute_field_stats_nested(
            vec![doc("a", &[("tags", arr_of_maps)])],
            "matched",
            5,
            true,
            3,
        );
        let keys: Vec<_> = rep.fields.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, vec!["tags"], "배열 내부는 펼쳐지지 않아야 한다");
        let stat = &rep.fields[0];
        assert!(
            stat.samples
                .iter()
                .all(|s| !s.value.contains("leak-me") && !s.value.contains("secret")),
            "배열 내부 본문이 sample에 노출되면 안 된다"
        );
    }

    #[test]
    fn nested_missing_counts_parent_absence() {
        // doc1: profile={email}, doc2: profile 없음.
        let rep = compute_field_stats_nested(
            vec![
                doc("a", &[("profile", map(&[("email", s("x@y.com"))]))]),
                doc("b", &[]),
            ],
            "matched",
            5,
            true,
            3,
        );
        let child = rep
            .fields
            .iter()
            .find(|f| f.key == "profile.email")
            .unwrap();
        assert_eq!(child.present, 1);
        assert_eq!(child.missing, 1, "부모 map이 없는 doc은 자식 키에서도 missing");
    }

    #[test]
    fn nested_accumulates_across_docs_with_same_path() {
        let rep = compute_field_stats_nested(
            vec![
                doc("a", &[("p", map(&[("q", s("x"))]))]),
                doc("b", &[("p", map(&[("q", s("y"))]))]),
                doc("c", &[("p", map(&[("q", s("x"))]))]),
            ],
            "matched",
            5,
            true,
            3,
        );
        let child = rep.fields.iter().find(|f| f.key == "p.q").unwrap();
        assert_eq!(child.present, 3);
        assert_eq!(child.missing, 0);
        // 동일 sample 값은 합산되어 count 내림차순.
        assert_eq!(child.samples[0].value, "x");
        assert_eq!(child.samples[0].count, 2);
        assert_eq!(child.samples[1].value, "y");
        assert_eq!(child.samples[1].count, 1);
    }

    #[test]
    fn nested_null_in_child_is_counted_at_child_level() {
        // 자식 값이 null인 경우 자식 row에 null_count로 반영, 부모는 present(map).
        let rep = compute_field_stats_nested(
            vec![doc(
                "a",
                &[("p", map(&[("q", FirestoreValue::Null)]))],
            )],
            "matched",
            5,
            true,
            3,
        );
        let parent = rep.fields.iter().find(|f| f.key == "p").unwrap();
        let child = rep.fields.iter().find(|f| f.key == "p.q").unwrap();
        assert_eq!(parent.null_count, 0, "부모는 map이므로 null 아님");
        assert_eq!(child.null_count, 1);
        assert_eq!(child.types[0].type_name, "null");
    }

    #[test]
    fn nested_pii_protection_parent_sample_still_summary_only() {
        // 부모 map row의 sample은 여전히 Map(n) 요약만 — 자식 본문 노출 차단.
        let rep = compute_field_stats_nested(
            vec![doc(
                "a",
                &[("p", map(&[("token", s("eyJhbGciOiJIUzI1NiJ9..."))]))],
            )],
            "matched",
            5,
            true,
            3,
        );
        let parent = rep.fields.iter().find(|f| f.key == "p").unwrap();
        assert_eq!(parent.samples[0].value, "Map(1)");
        assert!(
            parent
                .samples
                .iter()
                .all(|s| !s.value.contains("eyJhbGciOiJ")),
            "부모 map의 sample에 자식 본문이 새면 안 된다"
        );
    }

    #[test]
    fn nested_keys_remain_alpha_sorted_with_parents() {
        // dot-path 알파벳 순서. `a` < `a.b` < `b` < `b.c`.
        let rep = compute_field_stats_nested(
            vec![doc(
                "x",
                &[
                    ("b", map(&[("c", s("y"))])),
                    ("a", map(&[("b", s("x"))])),
                ],
            )],
            "matched",
            5,
            true,
            3,
        );
        let keys: Vec<_> = rep.fields.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "a.b", "b", "b.c"]);
    }

    #[test]
    fn clamp_max_depth_keeps_range() {
        assert_eq!(clamp_max_depth(0), 1, "0은 최소 1로 올린다");
        assert_eq!(clamp_max_depth(1), 1);
        assert_eq!(clamp_max_depth(3), 3);
        assert_eq!(clamp_max_depth(MAX_NESTED_DEPTH), MAX_NESTED_DEPTH);
        assert_eq!(clamp_max_depth(MAX_NESTED_DEPTH + 1), MAX_NESTED_DEPTH);
        assert_eq!(clamp_max_depth(usize::MAX), MAX_NESTED_DEPTH);
    }

    #[test]
    fn top_level_field_has_depth_zero_in_legacy_api() {
        // compute_field_stats(=legacy)도 새 depth 필드를 채워야 한다.
        let rep = compute_field_stats(vec![doc("a", &[("n", i(1))])], "matched", 5);
        assert_eq!(rep.fields[0].depth, 0);
    }
}
