// 컬렉션/문서/쿼리 IPC 래퍼 (`docs/03-ipc-contract.md` §3·§4).

import { call } from "./index";
import type { FirestoreDocument, QueryDsl } from "@/types";

export const listCollections = () =>
  call<string[]>("list_collections");

export const listSubcollections = (documentPath: string) =>
  call<string[]>("list_subcollections", { document_path: documentPath });

export const getDocument = (path: string) =>
  call<FirestoreDocument | null>("get_document", { path });

/** 즉시 반환 — 결과는 `query:chunk|done|error:<stream_id>` 이벤트로. */
export const queryDocuments = (streamId: string, dsl: QueryDsl) =>
  call<void>("query_documents", { stream_id: streamId, dsl });

export const cancelStream = (streamId: string) =>
  call<void>("cancel_stream", { stream_id: streamId });
