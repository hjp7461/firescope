//! 쿼리 결과 임시 디스크 sink (`docs/03-ipc-contract.md` §5 `export_result`).
//!
//! 설계 원칙:
//! - 결과는 메모리가 아닌 OS 임시 디렉터리의 NDJSON 파일에 누적된다
//!   (원칙 5 Secret Lifetime — 운영 데이터를 메모리에 잔존시키지 않음).
//! - sink는 `Drop` 시 임시 파일을 best-effort로 unlink한다.
//! - 한 파일에 *scanned* 전체를 기록하고 각 라인의 `matched: bool` 플래그로
//!   `source = matched|scanned` export를 분기한다. (디스크 사용량 절반)
//! - CSV 헤더 union을 위해 top-level 필드 키만 메모리에 set으로 유지한다
//!   (값은 디스크에만).

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::firestore::Document;

/// export 대상 (`docs/03-ipc-contract.md` §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportSource {
    /// post_filter 통과(매칭) 문서. 기본값.
    #[default]
    Matched,
    /// Firestore에서 받은 전체 문서 (후처리 이전).
    Scanned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Json,
    Ndjson,
    Csv,
}

#[derive(Serialize)]
struct SinkLine<'a> {
    matched: bool,
    doc: &'a Document,
}

#[derive(Deserialize)]
struct SinkLineOwned {
    matched: bool,
    doc: Document,
}

/// 단일 쿼리 스트림의 결과 임시 sink.
pub struct ResultSink {
    path: PathBuf,
    matched_count: usize,
    scanned_count: usize,
    field_keys: BTreeSet<String>,
}

impl ResultSink {
    /// 임시 디렉터리에 새 sink 파일을 생성한다.
    pub fn new() -> io::Result<Self> {
        Self::new_in(std::env::temp_dir())
    }

    pub fn new_in<P: AsRef<Path>>(dir: P) -> io::Result<Self> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("firescope-sink-{}.ndjson", Uuid::new_v4()));
        // 파일을 미리 만들어 둔다 (drop 시 unlink 대상 보장).
        OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)?;
        Ok(Self {
            path,
            matched_count: 0,
            scanned_count: 0,
            field_keys: BTreeSet::new(),
        })
    }

    // 외부 IPC에서는 query:done 이벤트의 total/scanned로 카운트를 제공하므로
    // 아래 카운터 접근자는 단위 테스트와 디버깅 용도다.
    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[allow(dead_code)]
    pub fn matched_count(&self) -> usize {
        self.matched_count
    }

    #[allow(dead_code)]
    pub fn scanned_count(&self) -> usize {
        self.scanned_count
    }

    #[allow(dead_code)]
    pub fn field_keys(&self) -> &BTreeSet<String> {
        &self.field_keys
    }

    /// scanned 한 건을 sink에 기록. `matched`가 true면 export "matched"에서도
    /// 노출되며, false면 "scanned"에서만 보인다.
    pub fn append(&mut self, doc: &Document, matched: bool) -> io::Result<()> {
        let mut f = OpenOptions::new().append(true).open(&self.path)?;
        let line = SinkLine { matched, doc };
        let bytes = serde_json::to_vec(&line).map_err(io::Error::other)?;
        f.write_all(&bytes)?;
        f.write_all(b"\n")?;
        self.scanned_count += 1;
        if matched {
            self.matched_count += 1;
            for k in doc.data.keys() {
                if !self.field_keys.contains(k) {
                    self.field_keys.insert(k.clone());
                }
            }
        }
        Ok(())
    }

    /// 지정 source에 해당하는 문서를 순회 반환 (스트리밍, 메모리 누적 없음).
    pub fn iter(
        &self,
        source: ExportSource,
    ) -> io::Result<impl Iterator<Item = io::Result<Document>>> {
        let f = File::open(&self.path)?;
        let reader = BufReader::new(f);
        Ok(reader.lines().filter_map(move |line| {
            let line = match line {
                Ok(l) => l,
                Err(e) => return Some(Err(e)),
            };
            if line.is_empty() {
                return None;
            }
            let parsed: SinkLineOwned = match serde_json::from_str(&line) {
                Ok(p) => p,
                Err(e) => return Some(Err(io::Error::other(e))),
            };
            if source == ExportSource::Matched && !parsed.matched {
                return None;
            }
            Some(Ok(parsed.doc))
        }))
    }

    /// JSON: `{ "docs": [...] }` 단일 객체.
    pub fn write_json(&self, out: &Path, source: ExportSource) -> io::Result<ExportStats> {
        let f = File::create(out)?;
        let mut w = BufWriter::new(f);
        w.write_all(b"{\"docs\":[")?;
        let mut row_count = 0usize;
        for (i, doc) in self.iter(source)?.enumerate() {
            let doc = doc?;
            if i > 0 {
                w.write_all(b",")?;
            }
            let s = serde_json::to_string(&doc).map_err(io::Error::other)?;
            w.write_all(s.as_bytes())?;
            row_count += 1;
        }
        w.write_all(b"]}\n")?;
        w.flush()?;
        let written_bytes = std::fs::metadata(out)?.len();
        Ok(ExportStats {
            written_bytes,
            row_count,
        })
    }

    /// NDJSON: 한 줄당 한 문서.
    pub fn write_ndjson(&self, out: &Path, source: ExportSource) -> io::Result<ExportStats> {
        let f = File::create(out)?;
        let mut w = BufWriter::new(f);
        let mut row_count = 0usize;
        for doc in self.iter(source)? {
            let doc = doc?;
            let s = serde_json::to_string(&doc).map_err(io::Error::other)?;
            w.write_all(s.as_bytes())?;
            w.write_all(b"\n")?;
            row_count += 1;
        }
        w.flush()?;
        let written_bytes = std::fs::metadata(out)?.len();
        Ok(ExportStats {
            written_bytes,
            row_count,
        })
    }

    /// CSV: `__path`, `__id` + 필드 union. nested는 JSON 문자열로 직렬화.
    ///
    /// `source = Scanned`에서는 matched 문서 외 필드도 포함하기 위해
    /// 헤더 계산용 1-pass + 본문 1-pass 총 2-pass를 한다(메모리에는 헤더만).
    pub fn write_csv(&self, out: &Path, source: ExportSource) -> io::Result<ExportStats> {
        let headers = match source {
            ExportSource::Matched => self.field_keys.iter().cloned().collect::<Vec<_>>(),
            ExportSource::Scanned => {
                let mut keys: BTreeSet<String> = self.field_keys.clone();
                for doc in self.iter(ExportSource::Scanned)? {
                    let doc = doc?;
                    for k in doc.data.keys() {
                        if !keys.contains(k) {
                            keys.insert(k.clone());
                        }
                    }
                }
                keys.into_iter().collect()
            }
        };

        let f = File::create(out)?;
        let mut w = BufWriter::new(f);
        // header row
        let mut header_cols = vec!["__path".to_string(), "__id".to_string()];
        header_cols.extend(headers.iter().cloned());
        write_csv_row(&mut w, &header_cols)?;

        let mut row_count = 0usize;
        for doc in self.iter(source)? {
            let doc = doc?;
            let mut row = Vec::with_capacity(header_cols.len());
            row.push(doc.path.clone());
            row.push(doc.id.clone());
            for k in &headers {
                let cell = match doc.data.get(k) {
                    None => String::new(),
                    Some(v) => firestore_value_to_csv_cell(v),
                };
                row.push(cell);
            }
            write_csv_row(&mut w, &row)?;
            row_count += 1;
        }
        w.flush()?;
        let written_bytes = std::fs::metadata(out)?.len();
        Ok(ExportStats {
            written_bytes,
            row_count,
        })
    }
}

impl Drop for ResultSink {
    fn drop(&mut self) {
        // best-effort: 운영 데이터를 디스크에 잔존시키지 않는다(원칙 5).
        let _ = std::fs::remove_file(&self.path);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ExportStats {
    pub written_bytes: u64,
    pub row_count: usize,
}

fn write_csv_row<W: Write>(w: &mut W, cells: &[String]) -> io::Result<()> {
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            w.write_all(b",")?;
        }
        w.write_all(escape_csv_cell(cell).as_bytes())?;
    }
    w.write_all(b"\r\n")?; // RFC 4180
    Ok(())
}

fn escape_csv_cell(s: &str) -> String {
    let needs_quote = s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r');
    if !needs_quote {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

fn firestore_value_to_csv_cell(v: &crate::query::dsl::FirestoreValue) -> String {
    use crate::query::dsl::FirestoreValue as FV;
    match v {
        FV::Null => String::new(),
        FV::Bool { value } => value.to_string(),
        FV::Int { value } => value.clone(),
        FV::Double { value } => value.to_string(),
        FV::String { value } => value.clone(),
        FV::Bytes { value } => value.clone(),
        FV::Timestamp { value } => value.clone(),
        FV::Reference { value } => value.clone(),
        FV::Geo { lat, lng } => format!("{lat},{lng}"),
        FV::Array { .. } | FV::Map { .. } => {
            serde_json::to_string(v).unwrap_or_else(|_| String::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::query::dsl::FirestoreValue;

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

    fn sink_dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!("firescope-sink-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn append_tracks_counts_and_keys() {
        let dir = sink_dir();
        let mut sink = ResultSink::new_in(&dir).unwrap();
        sink.append(
            &doc(
                "a",
                &[("n", FirestoreValue::Int { value: "1".into() })],
            ),
            true,
        )
        .unwrap();
        sink.append(
            &doc(
                "b",
                &[("m", FirestoreValue::Int { value: "2".into() })],
            ),
            false,
        )
        .unwrap();
        assert_eq!(sink.matched_count(), 1);
        assert_eq!(sink.scanned_count(), 2);
        // 헤더 union은 matched 기준 (CSV scanned export는 2-pass에서 보강)
        assert!(sink.field_keys().contains("n"));
        assert!(!sink.field_keys().contains("m"));
    }

    #[test]
    fn iter_filters_by_source() {
        let dir = sink_dir();
        let mut sink = ResultSink::new_in(&dir).unwrap();
        sink.append(&doc("a", &[]), true).unwrap();
        sink.append(&doc("b", &[]), false).unwrap();
        sink.append(&doc("c", &[]), true).unwrap();

        let matched: Vec<_> = sink
            .iter(ExportSource::Matched)
            .unwrap()
            .map(|r| r.unwrap().id)
            .collect();
        assert_eq!(matched, vec!["a", "c"]);

        let scanned: Vec<_> = sink
            .iter(ExportSource::Scanned)
            .unwrap()
            .map(|r| r.unwrap().id)
            .collect();
        assert_eq!(scanned, vec!["a", "b", "c"]);
    }

    #[test]
    fn write_json_writes_docs_object() {
        let dir = sink_dir();
        let mut sink = ResultSink::new_in(&dir).unwrap();
        sink.append(
            &doc(
                "a",
                &[("n", FirestoreValue::Int { value: "1".into() })],
            ),
            true,
        )
        .unwrap();
        let out = dir.join("out.json");
        let stats = sink.write_json(&out, ExportSource::Matched).unwrap();
        assert_eq!(stats.row_count, 1);
        assert!(stats.written_bytes > 0);
        let s = std::fs::read_to_string(&out).unwrap();
        assert!(s.starts_with("{\"docs\":["));
        assert!(s.contains("\"id\":\"a\""));
        assert!(s.trim_end().ends_with("]}"));
    }

    #[test]
    fn write_ndjson_one_per_line() {
        let dir = sink_dir();
        let mut sink = ResultSink::new_in(&dir).unwrap();
        sink.append(&doc("a", &[]), true).unwrap();
        sink.append(&doc("b", &[]), false).unwrap();
        sink.append(&doc("c", &[]), true).unwrap();

        let out = dir.join("out.ndjson");
        let stats = sink.write_ndjson(&out, ExportSource::Matched).unwrap();
        assert_eq!(stats.row_count, 2);
        let s = std::fs::read_to_string(&out).unwrap();
        let lines: Vec<_> = s.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"id\":\"a\""));
        assert!(lines[1].contains("\"id\":\"c\""));
    }

    #[test]
    fn write_csv_includes_path_id_and_field_union() {
        let dir = sink_dir();
        let mut sink = ResultSink::new_in(&dir).unwrap();
        sink.append(
            &doc(
                "a",
                &[
                    ("n", FirestoreValue::Int { value: "1".into() }),
                    (
                        "name",
                        FirestoreValue::String {
                            value: "Alice, the great".into(),
                        },
                    ),
                ],
            ),
            true,
        )
        .unwrap();
        sink.append(
            &doc("b", &[("n", FirestoreValue::Int { value: "2".into() })]),
            true,
        )
        .unwrap();

        let out = dir.join("out.csv");
        let stats = sink.write_csv(&out, ExportSource::Matched).unwrap();
        assert_eq!(stats.row_count, 2);
        let s = std::fs::read_to_string(&out).unwrap();
        let mut lines = s.lines();
        let header = lines.next().unwrap();
        assert!(header.starts_with("__path,__id,"));
        assert!(header.contains("n"));
        assert!(header.contains("name"));
        // 콤마 포함 셀은 따옴표 escape
        assert!(s.contains("\"Alice, the great\""));
    }

    #[test]
    fn drop_unlinks_temp_file() {
        let dir = sink_dir();
        let path = {
            let sink = ResultSink::new_in(&dir).unwrap();
            assert!(sink.path().exists());
            sink.path().to_path_buf()
        };
        assert!(!path.exists(), "sink temp file should be unlinked on drop");
    }

    #[test]
    fn csv_scanned_source_includes_unmatched_fields_in_header() {
        let dir = sink_dir();
        let mut sink = ResultSink::new_in(&dir).unwrap();
        sink.append(
            &doc(
                "a",
                &[("n", FirestoreValue::Int { value: "1".into() })],
            ),
            true,
        )
        .unwrap();
        sink.append(
            &doc(
                "b",
                &[(
                    "only_in_unmatched",
                    FirestoreValue::String {
                        value: "x".into(),
                    },
                )],
            ),
            false,
        )
        .unwrap();
        let out = dir.join("out.csv");
        sink.write_csv(&out, ExportSource::Scanned).unwrap();
        let header = std::fs::read_to_string(&out).unwrap();
        let first = header.lines().next().unwrap();
        assert!(first.contains("only_in_unmatched"));
        assert!(first.contains("n"));
    }
}
