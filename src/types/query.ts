// Rust `query::dsl` / `firestore::decode`와 동기화된 TS 타입
// (`docs/03-ipc-contract.md` 공통 타입, `docs/04-query-dsl.md`).

export type FirestoreValue =
  | { type: "null" }
  | { type: "bool"; value: boolean }
  | { type: "int"; value: string }
  | { type: "double"; value: number }
  | { type: "string"; value: string }
  | { type: "bytes"; value: string }
  | { type: "timestamp"; value: string }
  | { type: "reference"; value: string }
  | { type: "geo"; lat: number; lng: number }
  | { type: "array"; value: FirestoreValue[] }
  | { type: "map"; value: Record<string, FirestoreValue> };

export type FirestoreDocument = {
  path: string;
  id: string;
  parent: string;
  data: Record<string, FirestoreValue>;
  create_time: string | null;
  update_time: string | null;
};

export type QueryTarget =
  | { kind: "collection"; path: string }
  | { kind: "collection_group"; id: string };

export type CompareOp =
  | "=="
  | "!="
  | "<"
  | "<="
  | ">"
  | ">="
  | "array_contains"
  | "array_contains_any"
  | "in"
  | "not_in";

export type WhereClause = {
  field: string;
  op: CompareOp;
  value: FirestoreValue | FirestoreValue[];
};

export type OrderBy = { field: string; direction: "asc" | "desc" };

export type Cursor =
  | { kind: "document_ref"; path: string }
  | { kind: "values"; values: FirestoreValue[] };

export type RegexFilter = {
  fields: string[];
  pattern: string;
  case_insensitive?: boolean;
};

export type ContainsFilter = {
  fields: string[];
  text: string;
  case_insensitive?: boolean;
};

export type PostFilter = {
  regex?: RegexFilter;
  contains?: ContainsFilter;
  jsonpath?: string;
};

// `src-tauri/src/query/dsl.rs::QueryDsl`와 동기화 (`docs/04-query-dsl.md`).
export type QueryDsl = {
  target: QueryTarget;
  where?: WhereClause[];
  order_by?: OrderBy[];
  limit?: number;
  start_after?: Cursor;
  end_before?: Cursor;
  select?: string[];
  post_filter?: PostFilter;
};

// 쿼리 히스토리 (`docs/03-ipc-contract.md` §8).
export type QueryHistoryEntry = {
  id: string;
  dsl: QueryDsl;
  executed_at: string;
  took_ms?: number;
  result_count?: number;
};

// query_documents 이벤트 페이로드 (`query:chunk|done|error:<stream_id>`).
export type QueryChunk = { docs: FirestoreDocument[]; page: number };
export type QueryDone = {
  /** post_filter 통과(매칭) 건수 = 수신한 chunk 합계. */
  total: number;
  /** Firestore에서 가져온 전체 건수 (post_filter 없으면 total과 동일). */
  scanned: number;
  took_ms: number;
  has_more: boolean;
};

/** FirestoreValue를 셀 표시용 짧은 문자열로. */
export function renderValue(v: FirestoreValue): string {
  switch (v.type) {
    case "null":
      return "null";
    case "bool":
      return String(v.value);
    case "int":
    case "string":
    case "timestamp":
    case "reference":
    case "bytes":
      return v.value;
    case "double":
      return String(v.value);
    case "geo":
      return `(${v.lat}, ${v.lng})`;
    case "array":
      return `[${v.value.length}]`;
    case "map":
      return `{${Object.keys(v.value).length}}`;
  }
}
