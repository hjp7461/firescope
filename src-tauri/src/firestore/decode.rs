//! gcloud Firestore protobuf → DSL 값/문서 디코더.
//!
//! `docs/03-ipc-contract.md` 공통 타입(`Document`/`FirestoreValue`)으로 변환.
//! 모든 `ValueType` 변형을 빠짐없이 처리한다(부분 처리로 인한 조용한 손실 금지).

use std::collections::BTreeMap;

use chrono::DateTime;
use gcloud_sdk::google::firestore::v1::{value::ValueType, Document as PbDoc, Value as PbValue};
use gcloud_sdk::prost_types::Timestamp;
use serde::Serialize;

use crate::query::dsl::FirestoreValue;

/// IPC 응답 문서 (`docs/03-ipc-contract.md` `Document`).
#[derive(Debug, Clone, Serialize)]
pub struct Document {
    pub path: String,
    pub id: String,
    pub parent: String,
    pub data: BTreeMap<String, FirestoreValue>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
}

fn ts_to_rfc3339(ts: &Timestamp) -> Option<String> {
    DateTime::from_timestamp(ts.seconds, ts.nanos.max(0) as u32).map(|d| d.to_rfc3339())
}

/// 표준 base64 인코딩 (의존성 없이; bytes 값 표현용).
fn base64(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// 단일 protobuf `Value` → DSL `FirestoreValue` (재귀, 전 변형).
pub fn decode_value(v: &PbValue) -> FirestoreValue {
    match &v.value_type {
        None => FirestoreValue::Null,
        Some(vt) => match vt {
            ValueType::NullValue(_) => FirestoreValue::Null,
            ValueType::BooleanValue(b) => FirestoreValue::Bool { value: *b },
            ValueType::IntegerValue(n) => FirestoreValue::Int {
                value: n.to_string(),
            },
            ValueType::DoubleValue(d) => FirestoreValue::Double { value: *d },
            ValueType::StringValue(s) => FirestoreValue::String { value: s.clone() },
            ValueType::BytesValue(b) => FirestoreValue::Bytes { value: base64(b) },
            ValueType::TimestampValue(ts) => FirestoreValue::Timestamp {
                value: ts_to_rfc3339(ts).unwrap_or_default(),
            },
            ValueType::ReferenceValue(r) => FirestoreValue::Reference { value: r.clone() },
            ValueType::GeoPointValue(g) => FirestoreValue::Geo {
                lat: g.latitude,
                lng: g.longitude,
            },
            ValueType::ArrayValue(a) => FirestoreValue::Array {
                value: a.values.iter().map(decode_value).collect(),
            },
            ValueType::MapValue(m) => FirestoreValue::Map {
                value: m
                    .fields
                    .iter()
                    .map(|(k, v)| (k.clone(), decode_value(v)))
                    .collect(),
            },
        },
    }
}

/// protobuf `Document` → IPC `Document`.
///
/// `name`은 `projects/../databases/../documents/<path>` 형태 — `/documents/`
/// 뒤를 path로, 마지막 세그먼트를 id, 그 앞을 parent(컬렉션 경로)로 분리.
pub fn decode_document(doc: &PbDoc) -> Document {
    let path = doc
        .name
        .split_once("/documents/")
        .map(|(_, p)| p.to_string())
        .unwrap_or_else(|| doc.name.clone());

    let (parent, id) = match path.rsplit_once('/') {
        Some((p, last)) => (p.to_string(), last.to_string()),
        None => (String::new(), path.clone()),
    };

    Document {
        path,
        id,
        parent,
        data: doc
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), decode_value(v)))
            .collect(),
        create_time: doc.create_time.as_ref().and_then(ts_to_rfc3339),
        update_time: doc.update_time.as_ref().and_then(ts_to_rfc3339),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gcloud_sdk::google::firestore::v1::{ArrayValue, MapValue};

    fn v(vt: ValueType) -> PbValue {
        PbValue {
            value_type: Some(vt),
        }
    }

    #[test]
    fn scalars_decode() {
        assert_eq!(
            decode_value(&v(ValueType::IntegerValue(42))),
            FirestoreValue::Int { value: "42".into() }
        );
        assert_eq!(
            decode_value(&v(ValueType::BooleanValue(true))),
            FirestoreValue::Bool { value: true }
        );
        assert_eq!(
            decode_value(&PbValue { value_type: None }),
            FirestoreValue::Null
        );
    }

    #[test]
    fn base64_known_vector() {
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"f"), "Zg==");
    }

    #[test]
    fn nested_array_and_map() {
        let arr = v(ValueType::ArrayValue(ArrayValue {
            values: vec![v(ValueType::StringValue("a".into()))],
        }));
        match decode_value(&arr) {
            FirestoreValue::Array { value } => assert_eq!(value.len(), 1),
            _ => panic!("expected array"),
        }
        let mut fields = std::collections::HashMap::new();
        fields.insert("k".to_string(), v(ValueType::DoubleValue(1.5)));
        let map = v(ValueType::MapValue(MapValue { fields }));
        match decode_value(&map) {
            FirestoreValue::Map { value } => {
                assert!(matches!(
                    value.get("k"),
                    Some(FirestoreValue::Double { value }) if (*value - 1.5).abs() < 1e-9
                ));
            }
            _ => panic!("expected map"),
        }
    }

    #[test]
    fn document_name_splits_path_id_parent() {
        let doc = PbDoc {
            name: "projects/demo/databases/(default)/documents/users/abc123".into(),
            fields: Default::default(),
            create_time: None,
            update_time: None,
        };
        let d = decode_document(&doc);
        assert_eq!(d.path, "users/abc123");
        assert_eq!(d.id, "abc123");
        assert_eq!(d.parent, "users");
    }
}
