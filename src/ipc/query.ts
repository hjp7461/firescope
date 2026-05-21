// 컬렉션/문서/쿼리/히스토리 IPC 래퍼 (`docs/03-ipc-contract.md` §3·§4·§8).

import { call } from "./index";
import { getActiveSessionId } from "@/stores/tabsStore";

function requireActiveSessionId(): string {
  const id = getActiveSessionId();
  if (!id) throw new Error("no active session — activate a profile first");
  return id;
}
import type {
  ComputeStatsParams,
  ExportFormat,
  ExportResultResponse,
  ExportSource,
  FirestoreDocument,
  QueryCountResponse,
  QueryDsl,
  QueryHistoryEntry,
  StatsReport,
} from "@/types";

export const listCollections = () =>
  call<string[]>("list_collections", {
    session_id: requireActiveSessionId(),
  });

export const listSubcollections = (documentPath: string) =>
  call<string[]>("list_subcollections", {
    session_id: requireActiveSessionId(),
    document_path: documentPath,
  });

export interface ListCollectionDocIdsResponse {
  doc_ids: string[];
  page_token?: string;
}

/** 컬렉션 내 문서 ID만 가볍게 조회 (트리 네비게이션용). */
export const listCollectionDocIds = (params: {
  collection_id: string;
  parent_path?: string;
  page_size?: number;
  page_token?: string;
}) =>
  call<ListCollectionDocIdsResponse>("list_collection_doc_ids", {
    session_id: requireActiveSessionId(),
    ...params,
  });

export const getDocument = (path: string) =>
  call<FirestoreDocument | null>("get_document", {
    session_id: requireActiveSessionId(),
    path,
  });

/** 즉시 반환 — 결과는 `query:chunk|done|error:<stream_id>` 이벤트로. */
export const queryDocuments = (streamId: string, dsl: QueryDsl) =>
  call<void>("query_documents", {
    session_id: requireActiveSessionId(),
    stream_id: streamId,
    dsl,
  });

export const cancelStream = (streamId: string) =>
  call<void>("cancel_stream", { stream_id: streamId });

// --- Export / Count (`docs/03-ipc-contract.md` §4·§5 v0.5) ---

export const exportResult = (params: {
  stream_id: string;
  format: ExportFormat;
  path: string;
  source?: ExportSource;
}) => call<ExportResultResponse>("export_result", { params });

export const queryCount = (dsl: QueryDsl) =>
  call<QueryCountResponse>("query_count", {
    session_id: requireActiveSessionId(),
    dsl,
  });

// --- Phase 9: 컬렉션 통계 (`docs/03-ipc-contract.md` §5 v0.7) ---

export const computeStats = (params: ComputeStatsParams) =>
  call<StatsReport>("compute_stats", {
    session_id: requireActiveSessionId(),
    params,
  });

// --- 쿼리 히스토리 (`docs/03-ipc-contract.md` §8) ---

export const listQueryHistory = (profileId: string) =>
  call<QueryHistoryEntry[]>("list_query_history", { profile_id: profileId });

export const addQueryHistory = (params: {
  profile_id: string;
  dsl: QueryDsl;
  took_ms?: number;
  result_count?: number;
}) => call<QueryHistoryEntry>("add_query_history", { params });

export const removeQueryHistory = (profileId: string, entryId: string) =>
  call<void>("remove_query_history", {
    profile_id: profileId,
    entry_id: entryId,
  });

export const clearQueryHistory = (profileId: string) =>
  call<void>("clear_query_history", { profile_id: profileId });

export const pinQueryHistory = (
  profileId: string,
  entryId: string,
  pinned: boolean,
) =>
  call<QueryHistoryEntry>("pin_query_history", {
    profile_id: profileId,
    entry_id: entryId,
    pinned,
  });
