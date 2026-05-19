//! DSL → firestore `FirestoreQueryParams` 변환.
//!
//! 전체 비교 연산자(`==` `!=` `<` `<=` `>` `>=` `array_contains`
//! `array_contains_any` `in` `not_in`), 전체 값 타입, 값 기반 커서를 지원한다.
//! 표현 불가한 케이스(예: `document_ref` 커서)는 `AppError::InvalidQuery`로
//! 명시적으로 거부한다 (조용한 무시 금지). 호출 전 `query::validate`로
//! Firestore 제약·값 arity를 확인했다고 가정한다.

use chrono::DateTime;
use firestore::{
    FirestoreQueryCollection, FirestoreQueryCursor, FirestoreQueryDirection,
    FirestoreQueryFilter, FirestoreQueryFilterCompare, FirestoreQueryFilterComposite,
    FirestoreQueryFilterCompositeOperator, FirestoreQueryParams, FirestoreValue,
};
use gcloud_sdk::google::firestore::v1::{value::ValueType, ArrayValue, MapValue, Value};
use gcloud_sdk::google::r#type::LatLng;
use gcloud_sdk::prost_types::Timestamp;

use crate::error::{AppError, AppResult};
use crate::query::dsl::{
    CompareOp, Cursor, Direction, FirestoreValue as DslValue, QueryDsl, QueryTarget, WhereValue,
};

fn invalid(msg: impl Into<String>) -> AppError {
    AppError::InvalidQuery {
        message: msg.into(),
    }
}

/// 표준 base64 디코드 (의존성 없이; `decode.rs`의 인코더와 역연산).
fn base64_decode(s: &str) -> Result<Vec<u8>, ()> {
    fn sextet(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes = s.trim().as_bytes();
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if !bytes.len().is_multiple_of(4) {
        return Err(());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let mut buf = [0u8; 4];
        let mut pad = 0;
        for (i, &c) in chunk.iter().enumerate() {
            if c == b'=' {
                pad += 1;
            } else {
                if pad != 0 {
                    return Err(()); // '=' 뒤에 데이터 문자 금지
                }
                buf[i] = sextet(c).ok_or(())?;
            }
        }
        let n = (buf[0] as u32) << 18
            | (buf[1] as u32) << 12
            | (buf[2] as u32) << 6
            | buf[3] as u32;
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

/// DSL 값 → protobuf `Value` (전 타입, 재귀).
fn encode_value(v: &DslValue) -> AppResult<Value> {
    let value_type = match v {
        DslValue::Null => ValueType::NullValue(0),
        DslValue::Bool { value } => ValueType::BooleanValue(*value),
        DslValue::Int { value } => {
            let n: i64 = value
                .parse()
                .map_err(|_| invalid("int value is not a valid integer"))?;
            ValueType::IntegerValue(n)
        }
        DslValue::Double { value } => ValueType::DoubleValue(*value),
        DslValue::String { value } => ValueType::StringValue(value.clone()),
        DslValue::Bytes { value } => ValueType::BytesValue(
            base64_decode(value).map_err(|_| invalid("bytes value is not valid base64"))?,
        ),
        DslValue::Timestamp { value } => {
            let dt = DateTime::parse_from_rfc3339(value)
                .map_err(|_| invalid("timestamp value is not valid RFC3339"))?;
            ValueType::TimestampValue(Timestamp {
                seconds: dt.timestamp(),
                nanos: dt.timestamp_subsec_nanos() as i32,
            })
        }
        DslValue::Reference { value } => ValueType::ReferenceValue(value.clone()),
        DslValue::Geo { lat, lng } => ValueType::GeoPointValue(LatLng {
            latitude: *lat,
            longitude: *lng,
        }),
        DslValue::Array { value } => ValueType::ArrayValue(ArrayValue {
            values: value
                .iter()
                .map(encode_value)
                .collect::<AppResult<Vec<_>>>()?,
        }),
        DslValue::Map { value } => ValueType::MapValue(MapValue {
            fields: value
                .iter()
                .map(|(k, v)| Ok((k.clone(), encode_value(v)?)))
                .collect::<AppResult<_>>()?,
        }),
    };
    Ok(Value {
        value_type: Some(value_type),
    })
}

/// 멤버십 연산자(`in`/`not_in`/`array_contains_any`)의 값 목록을 단일
/// `ArrayValue` protobuf로 감싼다.
fn encode_array(items: &[DslValue]) -> AppResult<Value> {
    Ok(Value {
        value_type: Some(ValueType::ArrayValue(ArrayValue {
            values: items
                .iter()
                .map(encode_value)
                .collect::<AppResult<Vec<_>>>()?,
        })),
    })
}

fn to_compare(
    field: String,
    op: CompareOp,
    val: FirestoreValue,
) -> FirestoreQueryFilterCompare {
    use CompareOp::*;
    use FirestoreQueryFilterCompare as C;
    match op {
        Eq => C::Equal(field, val),
        Ne => C::NotEqual(field, val),
        Lt => C::LessThan(field, val),
        Le => C::LessThanOrEqual(field, val),
        Gt => C::GreaterThan(field, val),
        Ge => C::GreaterThanOrEqual(field, val),
        ArrayContains => C::ArrayContains(field, val),
        In => C::In(field, val),
        NotIn => C::NotIn(field, val),
        ArrayContainsAny => C::ArrayContainsAny(field, val),
    }
}

fn build_filter(dsl: &QueryDsl) -> AppResult<Option<FirestoreQueryFilter>> {
    if dsl.r#where.is_empty() {
        return Ok(None);
    }
    let mut compares = Vec::with_capacity(dsl.r#where.len());
    for w in &dsl.r#where {
        let val = match (&w.value, w.op.takes_array_value()) {
            (WhereValue::Many(items), true) => encode_array(items)?,
            (WhereValue::One(v), false) => encode_value(v)?,
            // validate가 선차단하지만 방어적으로 거부 (조용한 무시 금지).
            (_, true) => {
                return Err(invalid(format!("'{:?}' requires an array value", w.op)))
            }
            (_, false) => {
                return Err(invalid(format!(
                    "'{:?}' requires a single value, not an array",
                    w.op
                )))
            }
        };
        compares.push(FirestoreQueryFilter::Compare(Some(to_compare(
            w.field.clone(),
            w.op,
            FirestoreValue::from(val),
        ))));
    }

    Ok(Some(if compares.len() == 1 {
        compares.into_iter().next().unwrap()
    } else {
        FirestoreQueryFilter::Composite(FirestoreQueryFilterComposite {
            for_all_filters: compares,
            operator: FirestoreQueryFilterCompositeOperator::And,
        })
    }))
}

/// DSL 커서 → firestore 커서. `document_ref`는 정렬 필드값을 알 수 없어
/// 표현 불가 → 명시적으로 거부한다.
fn to_cursor(c: &Cursor) -> AppResult<Vec<FirestoreValue>> {
    match c {
        Cursor::Values { values } => values
            .iter()
            .map(|v| Ok(FirestoreValue::from(encode_value(v)?)))
            .collect(),
        Cursor::DocumentRef { .. } => Err(invalid(
            "document_ref cursor is not supported; use a values cursor",
        )),
    }
}

/// `(parent, collection_id, all_descendants)` 해석.
///
/// - collection "users"            → (None, Single("users"), false)
/// - collection "users/abc/posts"  → (Some("users/abc"), Single("posts"), false)
/// - collection_group "comments"   → (None, Group(["comments"]), true)
fn resolve_collection(
    target: &QueryTarget,
) -> AppResult<(Option<String>, FirestoreQueryCollection, bool)> {
    match target {
        QueryTarget::Collection { path } => {
            let trimmed = path.trim().trim_matches('/');
            let segments: Vec<&str> = trimmed.split('/').filter(|s| !s.is_empty()).collect();
            if segments.is_empty() {
                return Err(invalid("collection path is empty"));
            }
            // 컬렉션 경로는 세그먼트 수가 홀수여야 함 (doc은 짝수).
            if segments.len() % 2 == 0 {
                return Err(invalid("path points to a document, not a collection"));
            }
            let (last, parent) = segments.split_last().unwrap();
            let parent = if parent.is_empty() {
                None
            } else {
                Some(parent.join("/"))
            };
            Ok((
                parent,
                FirestoreQueryCollection::Single((*last).to_string()),
                false,
            ))
        }
        QueryTarget::CollectionGroup { id } => {
            if id.trim().is_empty() {
                return Err(invalid("collection group id is empty"));
            }
            Ok((
                None,
                FirestoreQueryCollection::Group(vec![id.trim().to_string()]),
                true,
            ))
        }
    }
}

/// 검증을 통과한 DSL을 firestore 쿼리 파라미터로 변환한다.
/// (호출 전 `query::validate`로 제약을 확인했다고 가정)
pub fn translate(dsl: &QueryDsl) -> AppResult<FirestoreQueryParams> {
    let (parent, collection_id, all_descendants) = resolve_collection(&dsl.target)?;

    let mut params = FirestoreQueryParams::new(collection_id);
    params.parent = parent;
    params.all_descendants = Some(all_descendants);
    params.limit = dsl.limit;
    params.filter = build_filter(dsl)?;

    if !dsl.order_by.is_empty() {
        let orders = dsl
            .order_by
            .iter()
            .map(|o| {
                let dir = match o.direction {
                    Direction::Asc => FirestoreQueryDirection::Ascending,
                    Direction::Desc => FirestoreQueryDirection::Descending,
                };
                (o.field.clone(), dir).into()
            })
            .collect();
        params.order_by = Some(orders);
    }

    if !dsl.select.is_empty() {
        params.return_only_fields = Some(dsl.select.clone());
    }

    if let Some(c) = &dsl.start_after {
        params.start_at = Some(FirestoreQueryCursor::AfterValue(to_cursor(c)?));
    }
    if let Some(c) = &dsl.end_before {
        params.end_at = Some(FirestoreQueryCursor::BeforeValue(to_cursor(c)?));
    }

    Ok(params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::dsl::{
        FirestoreValue as DslVal, OrderBy, QueryDsl, QueryTarget, WhereClause, WhereValue,
    };

    fn dsl(target: QueryTarget) -> QueryDsl {
        QueryDsl {
            target,
            r#where: vec![],
            order_by: vec![],
            limit: None,
            start_after: None,
            end_before: None,
            select: vec![],
            post_filter: None,
        }
    }

    #[test]
    fn nested_collection_splits_parent() {
        let (parent, col, all) = resolve_collection(&QueryTarget::Collection {
            path: "users/abc/posts".into(),
        })
        .unwrap();
        assert_eq!(parent.as_deref(), Some("users/abc"));
        assert!(matches!(col, FirestoreQueryCollection::Single(s) if s == "posts"));
        assert!(!all);
    }

    #[test]
    fn root_collection_no_parent() {
        let (parent, _, _) = resolve_collection(&QueryTarget::Collection {
            path: "users".into(),
        })
        .unwrap();
        assert!(parent.is_none());
    }

    #[test]
    fn document_path_rejected() {
        assert!(resolve_collection(&QueryTarget::Collection {
            path: "users/abc".into()
        })
        .is_err());
    }

    #[test]
    fn collection_group_all_descendants() {
        let (_, col, all) = resolve_collection(&QueryTarget::CollectionGroup {
            id: "comments".into(),
        })
        .unwrap();
        assert!(all);
        assert!(matches!(col, FirestoreQueryCollection::Group(_)));
    }

    #[test]
    fn eq_scalar_translates() {
        let mut q = dsl(QueryTarget::Collection {
            path: "users".into(),
        });
        q.r#where = vec![WhereClause {
            field: "active".into(),
            op: CompareOp::Eq,
            value: WhereValue::One(DslVal::Bool { value: true }),
        }];
        q.limit = Some(50);
        let p = translate(&q).unwrap();
        assert_eq!(p.limit, Some(50));
        assert!(p.filter.is_some());
    }

    fn one(target_field: &str, op: CompareOp, v: DslVal) -> QueryDsl {
        let mut q = dsl(QueryTarget::Collection {
            path: "users".into(),
        });
        q.r#where = vec![WhereClause {
            field: target_field.into(),
            op,
            value: WhereValue::One(v),
        }];
        q
    }

    #[test]
    fn all_comparison_operators_translate() {
        use firestore::FirestoreQueryFilter::Compare;
        use FirestoreQueryFilterCompare as C;
        let cases: Vec<(CompareOp, fn(&C) -> bool)> = vec![
            (CompareOp::Ne, |c| matches!(c, C::NotEqual(..))),
            (CompareOp::Lt, |c| matches!(c, C::LessThan(..))),
            (CompareOp::Le, |c| matches!(c, C::LessThanOrEqual(..))),
            (CompareOp::Gt, |c| matches!(c, C::GreaterThan(..))),
            (CompareOp::Ge, |c| matches!(c, C::GreaterThanOrEqual(..))),
            (CompareOp::ArrayContains, |c| {
                matches!(c, C::ArrayContains(..))
            }),
        ];
        for (op, check) in cases {
            let q = one("score", op, DslVal::Int { value: "10".into() });
            let p = translate(&q).unwrap();
            match p.filter {
                Some(Compare(Some(ref c))) => {
                    assert!(check(c), "{op:?} mapped to wrong compare variant")
                }
                _ => panic!("{op:?} did not translate to a compare filter"),
            }
        }
    }

    #[test]
    fn in_operator_translates_array_membership() {
        use firestore::FirestoreQueryFilter::Compare;
        let mut q = dsl(QueryTarget::Collection {
            path: "users".into(),
        });
        q.r#where = vec![WhereClause {
            field: "role".into(),
            op: CompareOp::In,
            value: WhereValue::Many(vec![
                DslVal::String { value: "a".into() },
                DslVal::String { value: "b".into() },
            ]),
        }];
        let p = translate(&q).unwrap();
        assert!(matches!(
            p.filter,
            Some(Compare(Some(FirestoreQueryFilterCompare::In(..))))
        ));
    }

    #[test]
    fn not_in_and_array_contains_any_translate() {
        use firestore::FirestoreQueryFilter::Compare;
        for (op, is_variant) in [
            (CompareOp::NotIn, 0u8),
            (CompareOp::ArrayContainsAny, 1u8),
        ] {
            let mut q = dsl(QueryTarget::Collection {
                path: "users".into(),
            });
            q.r#where = vec![WhereClause {
                field: "f".into(),
                op,
                value: WhereValue::Many(vec![DslVal::Int { value: "1".into() }]),
            }];
            let p = translate(&q).unwrap();
            match p.filter {
                Some(Compare(Some(FirestoreQueryFilterCompare::NotIn(..)))) if is_variant == 0 => {}
                Some(Compare(Some(FirestoreQueryFilterCompare::ArrayContainsAny(..))))
                    if is_variant == 1 => {}
                _ => panic!("{op:?} mapped to wrong variant"),
            }
        }
    }

    #[test]
    fn complex_values_translate() {
        // timestamp / reference / null / geo / bytes 를 == 비교로 변환
        for v in [
            DslVal::Timestamp {
                value: "2026-05-20T00:00:00Z".into(),
            },
            DslVal::Reference {
                value: "users/abc".into(),
            },
            DslVal::Null,
            DslVal::Geo {
                lat: 37.5,
                lng: 127.0,
            },
            DslVal::Bytes {
                value: "Zm9vYmFy".into(),
            },
        ] {
            let q = one("f", CompareOp::Eq, v.clone());
            assert!(translate(&q).is_ok(), "value {v:?} should translate");
        }
    }

    #[test]
    fn invalid_timestamp_rejected() {
        let q = one(
            "f",
            CompareOp::Eq,
            DslVal::Timestamp {
                value: "not-a-date".into(),
            },
        );
        assert!(translate(&q).is_err());
    }

    #[test]
    fn invalid_base64_bytes_rejected() {
        let q = one(
            "f",
            CompareOp::Eq,
            DslVal::Bytes {
                value: "!!!not-base64!!!".into(),
            },
        );
        assert!(translate(&q).is_err());
    }

    #[test]
    fn cursor_pagination_translates() {
        let mut q = dsl(QueryTarget::Collection {
            path: "users".into(),
        });
        q.order_by = vec![OrderBy {
            field: "created_at".into(),
            direction: Direction::Desc,
        }];
        q.start_after = Some(crate::query::dsl::Cursor::Values {
            values: vec![DslVal::Int { value: "100".into() }],
        });
        let p = translate(&q).unwrap();
        assert!(p.start_at.is_some());
    }
}
