//! 쿼리 DSL 타입 (`docs/04-query-dsl.md`).
//!
//! 순수 도메인: 프론트의 JSON DSL ↔ Rust. `tauri::*`/firestore 크레이트에
//! 의존하지 않는다 (검증은 `validate`, 변환은 `translate`가 담당).

use serde::{Deserialize, Serialize};

/// Firestore 값. `docs/03-ipc-contract.md` 공통 타입과 동일한 직렬화 형태
/// (`{ "type": "...", "value": ... }` 태그 유니온).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FirestoreValue {
    Null,
    Bool {
        value: bool,
    },
    /// i64를 문자열로 (JS 정밀도 손실 방지).
    Int {
        value: String,
    },
    Double {
        value: f64,
    },
    String {
        value: String,
    },
    /// base64
    Bytes {
        value: String,
    },
    Timestamp {
        value: String,
    },
    Reference {
        value: String,
    },
    Geo {
        lat: f64,
        lng: f64,
    },
    Array {
        value: Vec<FirestoreValue>,
    },
    Map {
        value: std::collections::BTreeMap<String, FirestoreValue>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueryTarget {
    Collection { path: String },
    CollectionGroup { id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareOp {
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "!=")]
    Ne,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    Ge,
    ArrayContains,
    ArrayContainsAny,
    In,
    NotIn,
}

impl CompareOp {
    /// 범위 비교(부등호)인가 — Firestore 단일 부등호 필드 제약 검증용.
    pub fn is_range(self) -> bool {
        matches!(
            self,
            CompareOp::Lt | CompareOp::Le | CompareOp::Gt | CompareOp::Ge
        )
    }

    /// 쿼리당 1회만 허용되는 분리(disjunctive) 연산자인가.
    pub fn is_singleton(self) -> bool {
        matches!(
            self,
            CompareOp::Ne | CompareOp::NotIn | CompareOp::ArrayContainsAny
        )
    }

    /// `value`가 배열이어야 하는 멤버십 연산자인가 (`in`/`not_in`/
    /// `array_contains_any`). 그 외는 단일 값만 허용한다.
    pub fn takes_array_value(self) -> bool {
        matches!(
            self,
            CompareOp::In | CompareOp::NotIn | CompareOp::ArrayContainsAny
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhereClause {
    pub field: String,
    pub op: CompareOp,
    pub value: WhereValue,
}

/// 단일 값 또는 배열(`in`/`array_contains_any`용).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WhereValue {
    One(FirestoreValue),
    Many(Vec<FirestoreValue>),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBy {
    pub field: String,
    pub direction: Direction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Cursor {
    DocumentRef { path: String },
    Values { values: Vec<FirestoreValue> },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PostFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regex: Option<RegexFilter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains: Option<ContainsFilter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jsonpath: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegexFilter {
    pub fields: Vec<String>,
    pub pattern: String,
    #[serde(default)]
    pub case_insensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainsFilter {
    pub fields: Vec<String>,
    pub text: String,
    #[serde(default)]
    pub case_insensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryDsl {
    pub target: QueryTarget,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub r#where: Vec<WhereClause>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order_by: Vec<OrderBy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_after: Option<Cursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_before: Option<Cursor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub select: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_filter: Option<PostFilter>,
}

/// Realtime 리스너용 DSL 서브셋 (Phase 11, `docs/03-ipc-contract.md` v0.10).
///
/// `QueryDsl`에서 `order_by`/`limit`/`select`/`cursor`/`post_filter`를
/// 제외한 형태. listener는 결과집합 전체를 스트리밍하므로 페이지네이션
/// 의미가 다르고, 후처리는 비용을 들이지 않는다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerDsl {
    pub target: QueryTarget,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub r#where: Vec<WhereClause>,
}

impl ListenerDsl {
    /// `validate`/`translate`가 받는 `QueryDsl`로 승격 — 다른 필드는 비어
    /// 있어 listener의 의미를 변경하지 않는다.
    pub fn to_query_dsl(&self) -> QueryDsl {
        QueryDsl {
            target: self.target.clone(),
            r#where: self.r#where.clone(),
            order_by: Vec::new(),
            limit: None,
            start_after: None,
            end_before: None,
            select: Vec::new(),
            post_filter: None,
        }
    }
}
