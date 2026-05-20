import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { cancelStream, queryDocuments } from "@/ipc/query";
import { getActiveSession } from "@/stores/tabsStore";
import { useHistoryStore } from "@/stores/historyStore";
import {
  type FirestoreDocument,
  type QueryChunk,
  type QueryDone,
  type QueryDsl,
  type QueryErrorPayload,
} from "@/types";
import { toKoreanMessage } from "@/lib/errorMessages";

type Status = "idle" | "streaming" | "done" | "error";

type ResultState = {
  streamId: string | null;
  collectionPath: string | null;
  /** 직전 실행한 DSL (히스토리 기록·재실행용). */
  lastDsl: QueryDsl | null;
  rows: FirestoreDocument[];
  status: Status;
  total: number;
  scanned: number;
  tookMs: number | null;
  error: string | null;
  /** Phase 8-A: Firestore 누락 인덱스 안내 URL (있을 때만). */
  indexUrl: string | null;
  runCollectionQuery: (path: string) => Promise<void>;
  runDsl: (dsl: QueryDsl) => Promise<void>;
  cancel: () => Promise<void>;
  reset: () => void;
};

/** dsl.target → ResultBar 표시용 라벨. */
function targetLabel(dsl: QueryDsl): string {
  return dsl.target.kind === "collection"
    ? dsl.target.path
    : `group:${dsl.target.id}`;
}

// 이벤트 unlisten 핸들은 store 밖(모듈 스코프)에 둔다 — 직렬화 대상 아님.
let unlisteners: UnlistenFn[] = [];

async function teardown() {
  const fns = unlisteners;
  unlisteners = [];
  await Promise.all(fns.map((f) => f()));
}

export const useResultStore = create<ResultState>((set, get) => ({
  streamId: null,
  collectionPath: null,
  lastDsl: null,
  rows: [],
  status: "idle",
  total: 0,
  scanned: 0,
  tookMs: null,
  error: null,
  indexUrl: null,

  reset: () => {
    void teardown();
    set({
      streamId: null,
      collectionPath: null,
      lastDsl: null,
      rows: [],
      status: "idle",
      total: 0,
      scanned: 0,
      tookMs: null,
      error: null,
      indexUrl: null,
    });
  },

  runDsl: async (dsl) => {
    await teardown();
    const streamId = crypto.randomUUID();
    set({
      streamId,
      collectionPath: targetLabel(dsl),
      lastDsl: dsl,
      rows: [],
      status: "streaming",
      total: 0,
      scanned: 0,
      tookMs: null,
      error: null,
      indexUrl: null,
    });

    // stream_id별 동적 이벤트 구독 (시작 전에 걸어 청크 유실 방지).
    unlisteners = await Promise.all([
      listen<QueryChunk>(`query:chunk:${streamId}`, (e) => {
        if (get().streamId !== streamId) return;
        set((s) => ({ rows: [...s.rows, ...e.payload.docs] }));
      }),
      listen<QueryDone>(`query:done:${streamId}`, (e) => {
        if (get().streamId !== streamId) return;
        set({
          status: "done",
          total: e.payload.total,
          scanned: e.payload.scanned,
          tookMs: e.payload.took_ms,
        });
        // 성공한 쿼리만 활성 프로파일 히스토리에 기록 (격리).
        const profileId = getActiveSession()?.profile_id;
        const ranDsl = get().lastDsl;
        if (profileId && ranDsl) {
          void useHistoryStore
            .getState()
            .record(profileId, ranDsl, e.payload.took_ms, e.payload.total);
        }
        void teardown();
      }),
      listen<QueryErrorPayload>(`query:error:${streamId}`, (e) => {
        if (get().streamId !== streamId) return;
        set({
          status: "error",
          error: toKoreanMessage(e.payload),
          indexUrl: e.payload.index_url ?? null,
        });
        void teardown();
      }),
    ]);

    try {
      await queryDocuments(streamId, dsl);
    } catch (err) {
      set({ status: "error", error: toKoreanMessage(err) });
      await teardown();
    }
  },

  runCollectionQuery: async (path) => {
    await get().runDsl({ target: { kind: "collection", path }, limit: 100 });
  },

  cancel: async () => {
    const id = get().streamId;
    if (!id) return;
    try {
      await cancelStream(id);
    } finally {
      set({ status: "done" });
      await teardown();
    }
  },
}));
