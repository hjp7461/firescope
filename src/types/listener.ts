// Realtime 리스너 타입 (`docs/03-ipc-contract.md` §8.5 v0.10).
// Rust `query::dsl::ListenerDsl` / `firestore::listener` 와 동기화.

import type { FirestoreDocument, QueryTarget, WhereClause } from "./query";

/**
 * Realtime listener DSL — `QueryDsl`의 서브셋.
 *
 * 의도적 제외: `order_by`/`limit`/`select`/`cursor`/`post_filter`.
 * Firestore listener는 결과집합 전체를 스트리밍하므로 페이지네이션 의미가
 * 다르고, 후처리는 컴파일 비용을 들이지 않는다.
 */
export type ListenerDsl = {
  target: QueryTarget;
  where?: WhereClause[];
};

/** `listener:change:<id>` 페이로드. */
export type ListenerChangePayload = {
  session_id: string;
  kind: "modified" | "removed";
  doc: FirestoreDocument;
};

/** `listener:status:<id>` 페이로드. */
export type ListenerStatusPayload = {
  session_id: string;
  status: "initial" | "ready" | "reset";
};

/** `list_listeners` 응답. */
export type ListenerInfo = {
  listener_id: string;
  session_id: string;
  target: QueryTarget;
  started_at: string;
  last_event_at?: string;
  event_count: number;
};
