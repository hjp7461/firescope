//! DSL → firestore `FirestoreQueryParams` 변환.
//!
//! Phase 2 범위(`docs/06-roadmap.md`): `==`/`limit`/`order_by`/select/
//! collection·collection_group 우선. 나머지 연산자·커서는 후속 Phase에서
//! 확장하며, 그때까지는 `AppError::InvalidQuery`로 명시적으로 거부한다
//! (조용한 무시 금지).

use firestore::{
    FirestoreQueryCollection, FirestoreQueryDirection, FirestoreQueryFilter,
    FirestoreQueryFilterCompare, FirestoreQueryFilterComposite,
    FirestoreQueryFilterCompositeOperator, FirestoreQueryParams, FirestoreValue,
};
use gcloud_sdk::google::firestore::v1::{value::ValueType, Value};

use crate::error::{AppError, AppResult};
use crate::query::dsl::{
    CompareOp, Direction, FirestoreValue as DslValue, QueryDsl, QueryTarget, WhereValue,
};

fn invalid(msg: impl Into<String>) -> AppError {
    AppError::InvalidQuery {
        message: msg.into(),
    }
}

/// DSL 스칼라 값 → firestore `FirestoreValue`.
///
/// Phase 2는 where 비교에 스칼라(null/bool/int/double/string)만 지원한다.
/// 복합 타입은 후속 Phase 전까지 거부한다.
fn scalar_to_value(v: &DslValue) -> AppResult<FirestoreValue> {
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
        _ => {
            return Err(invalid(
                "only scalar where values (null/bool/int/double/string) supported in Phase 2",
            ))
        }
    };
    Ok(FirestoreValue::from(Value {
        value_type: Some(value_type),
    }))
}

fn build_filter(dsl: &QueryDsl) -> AppResult<Option<FirestoreQueryFilter>> {
    if dsl.r#where.is_empty() {
        return Ok(None);
    }
    let mut compares = Vec::with_capacity(dsl.r#where.len());
    for w in &dsl.r#where {
        if w.op != CompareOp::Eq {
            return Err(invalid(
                "Phase 2 supports only the '==' operator in where clauses",
            ));
        }
        let val = match &w.value {
            WhereValue::One(v) => scalar_to_value(v)?,
            WhereValue::Many(_) => {
                return Err(invalid(
                    "array where value not supported with '==' (Phase 2)",
                ))
            }
        };
        compares.push(FirestoreQueryFilter::Compare(Some(
            FirestoreQueryFilterCompare::Equal(w.field.clone(), val),
        )));
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

    if dsl.start_after.is_some() || dsl.end_before.is_some() {
        return Err(invalid(
            "cursor pagination (start_after/end_before) is Phase 4+",
        ));
    }

    Ok(params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::dsl::{
        FirestoreValue as DslVal, QueryDsl, QueryTarget, WhereClause, WhereValue,
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
    fn non_eq_operator_rejected() {
        let mut q = dsl(QueryTarget::Collection {
            path: "users".into(),
        });
        q.r#where = vec![WhereClause {
            field: "age".into(),
            op: CompareOp::Gt,
            value: WhereValue::One(DslVal::Int { value: "1".into() }),
        }];
        assert!(translate(&q).is_err());
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
}
