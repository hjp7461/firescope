//! 결과 페이지네이션을 위한 cursor 산출 헬퍼.
//!
//! 정책 (1차):
//! - cursor는 항상 [`Cursor::Values`]로 만든다 (`DocumentRef`는 별도 PR로 미룸).
//! - `dsl.order_by`가 비어 있고 `limit`이 있으면 `__name__ asc`를 추가 정렬로
//!   보강한다 — 그렇지 않으면 cursor에 어느 필드의 값을 넣어야 할지 결정할
//!   수 없다. 사용자가 정렬을 명시한 경우에도 unique tiebreaker가 없으면
//!   Firestore가 같은 값의 문서를 중복/스킵할 수 있으므로 `__name__ asc`를
//!   tiebreaker로 항상 덧붙인다 (이미 들어 있으면 중복 추가 안 함).
//! - nested(dot-path) 정렬 필드는 cursor 값을 추출하지 못해 paging 비활성.
//!
//! 호출 흐름:
//! 1. `effective_order_by_fields(dsl)` → 페이지네이션에 사용할 필드 이름들.
//! 2. params.order_by에 이 필드들이 모두 포함되도록 streaming.rs에서 보강.
//! 3. 마지막 스캔 문서를 `compute_pagination_hint`에 넘겨 done 페이로드용
//!    `(has_more, cursor)`를 받는다.

use gcloud_sdk::google::firestore::v1::Document as PbDoc;

use crate::firestore::decode::decode_value;
use crate::query::dsl::{Cursor, FirestoreValue, QueryDsl};

/// Firestore가 doc id에 대해 예약한 정렬 필드명.
pub const NAME_FIELD: &str = "__name__";

/// dsl에서 paging 시 cursor가 매칭해야 할 필드 이름 목록 (정렬과 동일 순서).
///
/// - `dsl.order_by`의 필드들을 그대로 사용
/// - `dsl.limit`이 있으면 `__name__`를 마지막 tiebreaker로 추가 (이미 포함돼
///   있으면 추가하지 않음)
/// - `dsl.limit`이 없으면 paging 자체가 무의미하므로 빈 벡터
pub fn effective_order_by_fields(dsl: &QueryDsl) -> Vec<String> {
    if dsl.limit.is_none() {
        return Vec::new();
    }
    let mut fields: Vec<String> = dsl.order_by.iter().map(|o| o.field.clone()).collect();
    if !fields.iter().any(|f| f == NAME_FIELD) {
        fields.push(NAME_FIELD.to_string());
    }
    fields
}

/// `last_doc`로부터 cursor values를 추출한다.
///
/// 필드 중 하나라도 추출할 수 없으면 `None` (paging 강등).
/// 현재는 top-level scalar 필드만 지원 — nested(dot-path)는 미지원.
pub fn extract_cursor_values(
    last_doc: &PbDoc,
    fields: &[String],
) -> Option<Vec<FirestoreValue>> {
    let mut out = Vec::with_capacity(fields.len());
    for field in fields {
        if field == NAME_FIELD {
            out.push(FirestoreValue::Reference {
                value: last_doc.name.clone(),
            });
            continue;
        }
        if field.contains('.') {
            return None;
        }
        let v = last_doc.fields.get(field)?;
        out.push(decode_value(v));
    }
    Some(out)
}

/// `query:done` 페이로드의 `(has_more, cursor)` 산출.
///
/// 페이지네이션 가능 조건:
/// - 취소되지 않음
/// - `dsl.limit` 있음
/// - `scanned >= limit` (한 페이지를 가득 채움 → 더 있을 가능성)
/// - 마지막 스캔 문서 보유
/// - cursor 값 추출 성공
pub fn compute_pagination_hint(
    dsl: &QueryDsl,
    scanned: usize,
    cancelled: bool,
    last_doc: Option<&PbDoc>,
) -> (bool, Option<Cursor>) {
    if cancelled {
        return (false, None);
    }
    let Some(limit) = dsl.limit else {
        return (false, None);
    };
    if scanned < limit as usize {
        return (false, None);
    }
    let Some(doc) = last_doc else {
        return (false, None);
    };
    let fields = effective_order_by_fields(dsl);
    if fields.is_empty() {
        return (false, None);
    }
    match extract_cursor_values(doc, &fields) {
        Some(values) => (true, Some(Cursor::Values { values })),
        None => (false, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::dsl::{Direction, OrderBy, QueryTarget};
    use gcloud_sdk::google::firestore::v1::{value::ValueType, Value};
    use std::collections::HashMap;

    fn make_dsl(order: Vec<OrderBy>, limit: Option<u32>) -> QueryDsl {
        QueryDsl {
            target: QueryTarget::Collection {
                path: "Hospital".into(),
            },
            r#where: vec![],
            order_by: order,
            limit,
            start_after: None,
            end_before: None,
            select: vec![],
            post_filter: None,
        }
    }

    fn make_doc(name: &str, fields: Vec<(&str, ValueType)>) -> PbDoc {
        let mut map = HashMap::new();
        for (k, vt) in fields {
            map.insert(
                k.to_string(),
                Value {
                    value_type: Some(vt),
                },
            );
        }
        PbDoc {
            name: name.to_string(),
            fields: map,
            create_time: None,
            update_time: None,
        }
    }

    #[test]
    fn effective_fields_empty_when_no_limit() {
        let dsl = make_dsl(vec![], None);
        assert!(effective_order_by_fields(&dsl).is_empty());
    }

    #[test]
    fn effective_fields_uses_name_when_order_by_absent() {
        let dsl = make_dsl(vec![], Some(100));
        assert_eq!(effective_order_by_fields(&dsl), vec!["__name__"]);
    }

    #[test]
    fn effective_fields_appends_name_tiebreaker() {
        let dsl = make_dsl(
            vec![OrderBy {
                field: "createdDate".into(),
                direction: Direction::Desc,
            }],
            Some(100),
        );
        assert_eq!(
            effective_order_by_fields(&dsl),
            vec!["createdDate", "__name__"]
        );
    }

    #[test]
    fn effective_fields_does_not_duplicate_name() {
        let dsl = make_dsl(
            vec![OrderBy {
                field: "__name__".into(),
                direction: Direction::Asc,
            }],
            Some(100),
        );
        assert_eq!(effective_order_by_fields(&dsl), vec!["__name__"]);
    }

    #[test]
    fn extract_returns_name_reference_for_name_field() {
        let doc = make_doc("projects/p/databases/d/documents/Hospital/abc", vec![]);
        let v = extract_cursor_values(&doc, &["__name__".into()]).unwrap();
        assert_eq!(v.len(), 1);
        match &v[0] {
            FirestoreValue::Reference { value } => {
                assert_eq!(value, "projects/p/databases/d/documents/Hospital/abc");
            }
            other => panic!("expected Reference, got {other:?}"),
        }
    }

    #[test]
    fn extract_returns_top_level_scalar() {
        let doc = make_doc(
            "projects/p/databases/d/documents/Hospital/abc",
            vec![("createdDate", ValueType::IntegerValue(1724736092751))],
        );
        let v = extract_cursor_values(&doc, &["createdDate".into(), "__name__".into()]).unwrap();
        match &v[0] {
            FirestoreValue::Int { value } => assert_eq!(value, "1724736092751"),
            other => panic!("expected Int, got {other:?}"),
        }
        match &v[1] {
            FirestoreValue::Reference { value } => {
                assert_eq!(value, "projects/p/databases/d/documents/Hospital/abc");
            }
            other => panic!("expected Reference, got {other:?}"),
        }
    }

    #[test]
    fn extract_fails_for_missing_field() {
        let doc = make_doc("name", vec![]);
        assert!(extract_cursor_values(&doc, &["missing".into()]).is_none());
    }

    #[test]
    fn extract_fails_for_nested_path() {
        let doc = make_doc("name", vec![("a", ValueType::IntegerValue(1))]);
        assert!(extract_cursor_values(&doc, &["a.b".into()]).is_none());
    }

    #[test]
    fn hint_false_when_cancelled() {
        let dsl = make_dsl(vec![], Some(100));
        let doc = make_doc("n", vec![]);
        assert_eq!(
            compute_pagination_hint(&dsl, 100, true, Some(&doc)),
            (false, None)
        );
    }

    #[test]
    fn hint_false_when_no_limit() {
        let dsl = make_dsl(vec![], None);
        let doc = make_doc("n", vec![]);
        assert_eq!(
            compute_pagination_hint(&dsl, 100, false, Some(&doc)),
            (false, None)
        );
    }

    #[test]
    fn hint_false_when_scanned_below_limit() {
        let dsl = make_dsl(vec![], Some(100));
        let doc = make_doc("n", vec![]);
        assert_eq!(
            compute_pagination_hint(&dsl, 50, false, Some(&doc)),
            (false, None)
        );
    }

    #[test]
    fn hint_true_with_name_cursor_when_no_order_by() {
        let dsl = make_dsl(vec![], Some(100));
        let doc = make_doc("projects/p/databases/d/documents/Hospital/abc", vec![]);
        let (has_more, cursor) = compute_pagination_hint(&dsl, 100, false, Some(&doc));
        assert!(has_more);
        let cursor = cursor.expect("cursor present");
        match cursor {
            Cursor::Values { values } => {
                assert_eq!(values.len(), 1);
                assert!(matches!(values[0], FirestoreValue::Reference { .. }));
            }
            other => panic!("expected Values cursor, got {other:?}"),
        }
    }

    #[test]
    fn hint_includes_both_order_field_and_name_tiebreaker() {
        let dsl = make_dsl(
            vec![OrderBy {
                field: "createdDate".into(),
                direction: Direction::Desc,
            }],
            Some(100),
        );
        let doc = make_doc(
            "projects/p/databases/d/documents/Hospital/abc",
            vec![("createdDate", ValueType::IntegerValue(42))],
        );
        let (has_more, cursor) = compute_pagination_hint(&dsl, 100, false, Some(&doc));
        assert!(has_more);
        if let Some(Cursor::Values { values }) = cursor {
            assert_eq!(values.len(), 2);
            assert!(matches!(values[0], FirestoreValue::Int { .. }));
            assert!(matches!(values[1], FirestoreValue::Reference { .. }));
        } else {
            panic!("expected Values cursor");
        }
    }

    #[test]
    fn hint_downgrades_to_false_when_extraction_fails() {
        let dsl = make_dsl(
            vec![OrderBy {
                field: "nested.field".into(),
                direction: Direction::Asc,
            }],
            Some(100),
        );
        let doc = make_doc("n", vec![]);
        assert_eq!(
            compute_pagination_hint(&dsl, 100, false, Some(&doc)),
            (false, None)
        );
    }
}
