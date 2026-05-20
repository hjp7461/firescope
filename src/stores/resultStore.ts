import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { cancelStream, queryDocuments } from "@/ipc/query";
import { getActiveSession, registerTabCloseCleanup, useTabsStore, type TabId } from "@/stores/tabsStore";
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

export type ResultSlice = {
  streamId: string | null;
  collectionPath: string | null;
  lastDsl: QueryDsl | null;
  rows: FirestoreDocument[];
  status: Status;
  total: number;
  scanned: number;
  tookMs: number | null;
  error: string | null;
  indexUrl: string | null;
};

const EMPTY_SLICE: ResultSlice = {
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
};

type ResultState = ResultSlice & {
  byTab: Map<TabId, ResultSlice>;
  runCollectionQuery: (path: string) => Promise<void>;
  runDsl: (dsl: QueryDsl) => Promise<void>;
  cancel: () => Promise<void>;
  reset: () => void;
  dropTab: (tabId: TabId) => void;

  __resetForTests: () => void;
  __setSliceForTest: (tabId: TabId, slice: ResultSlice) => void;
  __registerStreamForTest: (streamId: string, tabId: TabId) => void;
  __getTabForStream: (streamId: string) => TabId | undefined;
};

/** dsl.target → ResultBar 표시용 라벨. */
function targetLabel(dsl: QueryDsl): string {
  return dsl.target.kind === "collection"
    ? dsl.target.path
    : `group:${dsl.target.id}`;
}

// 이벤트 unlisten 핸들은 store 밖(모듈 스코프)에 둔다 — 직렬화 대상 아님.
// stream-id별로 unlisten을 추적하여 다중 in-flight 스트림 가능.
const streamIdToTab = new Map<string, TabId>();
const unlisteners = new Map<string, UnlistenFn[]>();

async function teardownStream(streamId: string) {
  const fns = unlisteners.get(streamId) ?? [];
  unlisteners.delete(streamId);
  streamIdToTab.delete(streamId);
  await Promise.all(fns.map((f) => f()));
}

function setSlice(
  state: ResultState,
  tabId: TabId,
  patch: Partial<ResultSlice>,
): ResultState {
  const prev = state.byTab.get(tabId) ?? EMPTY_SLICE;
  const next: ResultSlice = { ...prev, ...patch };
  const map = new Map(state.byTab);
  map.set(tabId, next);
  const isActive = useTabsStore.getState().activeTabId === tabId;
  return isActive
    ? { ...state, ...next, byTab: map }
    : { ...state, byTab: map };
}

export const useResultStore = create<ResultState>((set, get) => ({
  ...EMPTY_SLICE,
  byTab: new Map(),

  reset: () => {
    const tabId = useTabsStore.getState().activeTabId;
    if (!tabId) return;
    const sid = get().byTab.get(tabId)?.streamId;
    if (sid) void teardownStream(sid);
    set((s) => setSlice(s, tabId, { ...EMPTY_SLICE }));
  },

  runDsl: async (dsl) => {
    const tabId = useTabsStore.getState().activeTabId;
    if (!tabId) return;
    const prevStreamId = get().byTab.get(tabId)?.streamId;
    if (prevStreamId) await teardownStream(prevStreamId);

    const streamId = crypto.randomUUID();
    streamIdToTab.set(streamId, tabId);
    set((s) =>
      setSlice(s, tabId, {
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
      }),
    );

    // stream_id별 동적 이벤트 구독 (시작 전에 걸어 청크 유실 방지).
    const dynamicListeners = await Promise.all([
      listen<QueryChunk>(`query:chunk:${streamId}`, (e) => {
        const owner = streamIdToTab.get(streamId);
        if (!owner) return;
        const prev = get().byTab.get(owner)?.rows ?? [];
        set((s) =>
          setSlice(s, owner, { rows: [...prev, ...e.payload.docs] }),
        );
      }),
      listen<QueryDone>(`query:done:${streamId}`, (e) => {
        const owner = streamIdToTab.get(streamId);
        if (!owner) return;
        set((s) =>
          setSlice(s, owner, {
            status: "done",
            total: e.payload.total,
            scanned: e.payload.scanned,
            tookMs: e.payload.took_ms,
          }),
        );
        // 성공한 쿼리만 활성 프로파일 히스토리에 기록 (격리).
        const profileId = getActiveSession()?.profile_id;
        const ranDsl = get().byTab.get(owner)?.lastDsl;
        if (profileId && ranDsl) {
          void useHistoryStore
            .getState()
            .record(profileId, ranDsl, e.payload.took_ms, e.payload.total);
        }
        void teardownStream(streamId);
      }),
      listen<QueryErrorPayload>(`query:error:${streamId}`, (e) => {
        const owner = streamIdToTab.get(streamId);
        if (!owner) return;
        set((s) =>
          setSlice(s, owner, {
            status: "error",
            error: toKoreanMessage(e.payload),
            indexUrl: e.payload.index_url ?? null,
          }),
        );
        void teardownStream(streamId);
      }),
    ]);
    unlisteners.set(streamId, dynamicListeners);

    try {
      await queryDocuments(streamId, dsl);
    } catch (err) {
      set((s) => setSlice(s, tabId, { status: "error", error: toKoreanMessage(err) }));
      await teardownStream(streamId);
    }
  },

  runCollectionQuery: async (path) => {
    await get().runDsl({ target: { kind: "collection", path }, limit: 100 });
  },

  cancel: async () => {
    const tabId = useTabsStore.getState().activeTabId;
    if (!tabId) return;
    const sid = get().byTab.get(tabId)?.streamId;
    if (!sid) return;
    try {
      await cancelStream(sid);
    } finally {
      set((s) => setSlice(s, tabId, { status: "done" }));
      await teardownStream(sid);
    }
  },

  dropTab: (tabId) => {
    const owned: string[] = [];
    for (const [streamId, owner] of streamIdToTab.entries()) {
      if (owner === tabId) owned.push(streamId);
    }
    for (const streamId of owned) {
      void teardownStream(streamId);
    }
    set((s) => {
      if (!s.byTab.has(tabId)) return s;
      const map = new Map(s.byTab);
      map.delete(tabId);
      return { ...s, byTab: map };
    });
  },

  __resetForTests: () => {
    streamIdToTab.clear();
    unlisteners.clear();
    set({ ...EMPTY_SLICE, byTab: new Map() });
  },
  __setSliceForTest: (tabId, slice) => {
    set((s) => setSlice(s, tabId, slice));
  },
  __registerStreamForTest: (streamId, tabId) => {
    streamIdToTab.set(streamId, tabId);
  },
  __getTabForStream: (streamId) => streamIdToTab.get(streamId),
}));

// 활성 탭 변경 시 top-level 미러 동기화 (Zustand subscribe — 기본 형식)
useTabsStore.subscribe((s, prev) => {
  if (s.activeTabId === prev.activeTabId) return;
  const slice = s.activeTabId
    ? useResultStore.getState().byTab.get(s.activeTabId) ?? EMPTY_SLICE
    : EMPTY_SLICE;
  useResultStore.setState(slice);
});

registerTabCloseCleanup((tabId) => {
  useResultStore.getState().dropTab(tabId);
});
