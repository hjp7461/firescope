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

export type QueryDsl = {
  target: QueryTarget;
  where?: WhereClause[];
  order_by?: OrderBy[];
  limit?: number;
  select?: string[];
};

// query_documents 이벤트 페이로드 (`query:chunk|done|error:<stream_id>`).
export type QueryChunk = { docs: FirestoreDocument[]; page: number };
export type QueryDone = {
  total: number;
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
