//! 쿼리 DSL 검증 (`docs/04-query-dsl.md` "검증 규칙").
//!
//! 순수 함수 — `tauri::*`/firestore 의존 없음. 위반 시 `AppError::InvalidQuery`.
//! Firestore 네이티브 제약을 *사전* 검증해 무의미한 RPC를 막는다.

use crate::error::{AppError, AppResult};
use crate::query::dsl::{CompareOp, QueryDsl, QueryTarget};

const MAX_WHERE: usize = 30;
const MAX_LIMIT: u32 = 1000;

fn invalid(msg: impl Into<String>) -> AppError {
    AppError::InvalidQuery {
        message: msg.into(),
    }
}

pub fn validate(dsl: &QueryDsl) -> AppResult<()> {
    // 1) target 비어있지 않음
    match &dsl.target {
        QueryTarget::Collection { path } if path.trim().is_empty() => {
            return Err(invalid("collection path is empty"));
        }
        QueryTarget::CollectionGroup { id } if id.trim().is_empty() => {
            return Err(invalid("collection group id is empty"));
        }
        _ => {}
    }

    // 2) where 길이 ≤ 30
    if dsl.r#where.len() > MAX_WHERE {
        return Err(invalid(format!(
            "too many where clauses: {} (max {MAX_WHERE})",
            dsl.r#where.len()
        )));
    }

    // 3) 부등호(범위) 필드 ≤ 1, 4) 분리 연산자 결합 ≤ 1
    let mut range_field: Option<&str> = None;
    let mut singleton_count = 0;
    for w in &dsl.r#where {
        if w.op.is_range() {
            match range_field {
                Some(f) if f != w.field => {
                    return Err(invalid(format!(
                        "range operators on multiple fields not allowed ('{f}' and '{}')",
                        w.field
                    )));
                }
                _ => range_field = Some(&w.field),
            }
        }
        if w.op.is_singleton() {
            singleton_count += 1;
        }
    }
    if singleton_count > 1 {
        return Err(invalid(
            "at most one of !=, not_in, array_contains_any per query",
        ));
    }

    // (Firestore) order_by 첫 필드는 범위 비교 필드와 동일해야 함
    if let (Some(rf), Some(first)) = (range_field, dsl.order_by.first()) {
        if first.field != rf {
            return Err(invalid(format!(
                "first order_by field must match range field '{rf}'"
            )));
        }
    }

    // 5) limit 범위 1..=1000
    if let Some(limit) = dsl.limit {
        if limit == 0 || limit > MAX_LIMIT {
            return Err(invalid(format!("limit must be 1..={MAX_LIMIT}")));
        }
    }

    // 6) select 필드명에 부적절 문자 없음
    for f in &dsl.select {
        if f.contains("..") || f.chars().any(|c| c.is_whitespace()) || f.is_empty() {
            return Err(invalid(format!("invalid select field: '{f}'")));
        }
    }

    // 7) post_filter.regex.pattern 컴파일 가능
    if let Some(pf) = &dsl.post_filter {
        if let Some(rx) = &pf.regex {
            regex::Regex::new(&rx.pattern)
                .map_err(|_| invalid("post_filter regex pattern does not compile"))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::dsl::*;

    fn base() -> QueryDsl {
        QueryDsl {
            target: QueryTarget::Collection {
                path: "users".into(),
            },
            r#where: vec![],
            order_by: vec![],
            limit: None,
            start_after: None,
            end_before: None,
            select: vec![],
            post_filter: None,
        }
    }

    fn w(field: &str, op: CompareOp) -> WhereClause {
        WhereClause {
            field: field.into(),
            op,
            value: WhereValue::One(FirestoreValue::Bool { value: true }),
        }
    }

    #[test]
    fn ok_minimal() {
        assert!(validate(&base()).is_ok());
    }

    #[test]
    fn empty_target_rejected() {
        let mut q = base();
        q.target = QueryTarget::Collection { path: "  ".into() };
        assert!(validate(&q).is_err());
    }

    #[test]
    fn limit_bounds() {
        let mut q = base();
        q.limit = Some(0);
        assert!(validate(&q).is_err());
        q.limit = Some(1001);
        assert!(validate(&q).is_err());
        q.limit = Some(100);
        assert!(validate(&q).is_ok());
    }

    #[test]
    fn multi_field_range_rejected() {
        let mut q = base();
        q.r#where = vec![w("a", CompareOp::Gt), w("b", CompareOp::Lt)];
        assert!(validate(&q).is_err());
    }

    #[test]
    fn single_field_range_ok() {
        let mut q = base();
        q.r#where = vec![w("a", CompareOp::Gt), w("a", CompareOp::Le)];
        assert!(validate(&q).is_ok());
    }

    #[test]
    fn double_singleton_rejected() {
        let mut q = base();
        q.r#where = vec![w("a", CompareOp::Ne), w("b", CompareOp::NotIn)];
        assert!(validate(&q).is_err());
    }

    #[test]
    fn order_by_must_match_range_field() {
        let mut q = base();
        q.r#where = vec![w("score", CompareOp::Ge)];
        q.order_by = vec![OrderBy {
            field: "name".into(),
            direction: Direction::Asc,
        }];
        assert!(validate(&q).is_err());
        q.order_by = vec![OrderBy {
            field: "score".into(),
            direction: Direction::Desc,
        }];
        assert!(validate(&q).is_ok());
    }

    #[test]
    fn bad_select_field_rejected() {
        let mut q = base();
        q.select = vec!["a..b".into()];
        assert!(validate(&q).is_err());
    }

    #[test]
    fn uncompilable_regex_rejected() {
        let mut q = base();
        q.post_filter = Some(PostFilter {
            regex: Some(RegexFilter {
                fields: vec!["name".into()],
                pattern: "(".into(),
                case_insensitive: false,
            }),
            contains: None,
            jsonpath: None,
        });
        assert!(validate(&q).is_err());
    }
}
