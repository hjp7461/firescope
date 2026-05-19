import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { cancelStream, queryDocuments } from "@/ipc/query";
import {
  asAppError,
  type FirestoreDocument,
  type QueryChunk,
  type QueryDone,
  type QueryDsl,
} from "@/types";

type Status = "idle" | "streaming" | "done" | "error";

type ResultState = {
  streamId: string | null;
  collectionPath: string | null;
  rows: FirestoreDocument[];
  status: Status;
  total: number;
  tookMs: number | null;
  error: string | null;
  runCollectionQuery: (path: string) => Promise<void>;
  cancel: () => Promise<void>;
  reset: () => void;
};

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
  rows: [],
  status: "idle",
  total: 0,
  tookMs: null,
  error: null,

  reset: () => {
    void teardown();
    set({
      streamId: null,
      collectionPath: null,
      rows: [],
      status: "idle",
      total: 0,
      tookMs: null,
      error: null,
    });
  },

  runCollectionQuery: async (path) => {
    await teardown();
    const streamId = crypto.randomUUID();
    set({
      streamId,
      collectionPath: path,
      rows: [],
      status: "streaming",
      total: 0,
      tookMs: null,
      error: null,
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
          tookMs: e.payload.took_ms,
        });
        void teardown();
      }),
      listen<{ kind: string; message: string }>(
        `query:error:${streamId}`,
        (e) => {
          if (get().streamId !== streamId) return;
          set({ status: "error", error: e.payload.message });
          void teardown();
        },
      ),
    ]);

    const dsl: QueryDsl = {
      target: { kind: "collection", path },
      limit: 100,
    };
    try {
      await queryDocuments(streamId, dsl);
    } catch (err) {
      set({ status: "error", error: asAppError(err).message });
      await teardown();
    }
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
