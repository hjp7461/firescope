import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { cancelStream, getDocument, queryDocuments } from "@/ipc/query";
import { startListener, stopListener } from "@/ipc/listener";
import { getActiveSession, registerTabCloseCleanup, useTabsStore, type TabId } from "@/stores/tabsStore";
import { useHistoryStore } from "@/stores/historyStore";
import {
  type Cursor,
  type FirestoreDocument,
  type ListenerChangePayload,
  type ListenerDsl,
  type ListenerStatusPayload,
  type QueryChunk,
  type QueryDone,
  type QueryDsl,
  type QueryErrorPayload,
} from "@/types";
import { toKoreanMessage } from "@/lib/errorMessages";

type Status = "idle" | "streaming" | "done" | "error" | "listening";
type ListenerStatus = "initial" | "ready" | "reset";

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
  /** 활성 listener id. 없으면 null (= 스냅샷 모드). */
  listenerId: string | null;
  /** 마지막 `listener:status` 이벤트 — `null`이면 listener 비활성. */
  listenerStatus: ListenerStatus | null;
  /** listener 모드에서 누적된 변경 이벤트 수. */
  listenerEventCount: number;
  /** 백엔드가 다음 페이지를 줄 수 있다고 알린 상태 (v0.11). */
  hasMore: boolean;
  /** 다음 페이지 요청 시 그대로 `start_after`로 보낼 cursor. */
  nextCursor: Cursor | null;
  /** fetchMore가 진행 중인지 — 동시 트리거 방지 + UI 인디케이터용. */
  fetchMoreInFlight: boolean;
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
  listenerId: null,
  listenerStatus: null,
  listenerEventCount: 0,
  hasMore: false,
  nextCursor: null,
  fetchMoreInFlight: false,
};

type ResultState = ResultSlice & {
  byTab: Map<TabId, ResultSlice>;
  runCollectionQuery: (path: string) => Promise<void>;
  runDsl: (dsl: QueryDsl) => Promise<void>;
  /** 단일 문서를 가져와 결과 패널에 1행으로 표시 (트리에서 문서 노드 선택용). */
  selectDocument: (path: string) => Promise<void>;
  /** 마지막 페이지의 cursor로 같은 DSL을 재실행해 다음 100건을 누적한다. */
  fetchMore: () => Promise<void>;
  cancel: () => Promise<void>;
  reset: () => void;
  dropTab: (tabId: TabId) => void;

  /** Realtime listener를 시작한다 (Phase 11). 기존 스트림/listener는 정리. */
  startListening: (dsl: ListenerDsl) => Promise<void>;
  /** 활성 listener를 중지하고 스냅샷 모드로 복귀한다. */
  stopListening: () => Promise<void>;

  __resetForTests: () => void;
  __setSliceForTest: (tabId: TabId, slice: Partial<ResultSlice>) => void;
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

// Realtime listener — listener-id별 unlisten 추적 (스트림과 분리).
const listenerIdToTab = new Map<string, TabId>();
const listenerUnlisteners = new Map<string, UnlistenFn[]>();

async function teardownListener(listenerId: string) {
  const fns = listenerUnlisteners.get(listenerId) ?? [];
  listenerUnlisteners.delete(listenerId);
  listenerIdToTab.delete(listenerId);
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

/**
 * chunk/done/error 이벤트 리스너를 streamId에 묶는다 (runDsl/fetchMore 공유).
 *
 * - `replace` 모드: done 시 total/scanned를 페이로드 그대로 세팅하고 첫
 *   실행이므로 히스토리에 기록.
 * - `append` 모드: done 시 기존 total/scanned에 누적하고 히스토리 기록은
 *   건너뛴다 (같은 쿼리의 연속 페이지이므로 중복 방지).
 *
 * chunk 처리는 두 모드 모두 동일하게 기존 rows에 APPEND한다 — replace 모드는
 * runDsl 시작 시 rows를 []로 초기화하므로 append 시작점이 비어 있을 뿐이다.
 */
async function attachStreamListeners(
  streamId: string,
  mode: "replace" | "append",
): Promise<UnlistenFn[]> {
  return Promise.all([
    listen<QueryChunk>(`query:chunk:${streamId}`, (e) => {
      const owner = streamIdToTab.get(streamId);
      if (!owner) return;
      const prev = useResultStore.getState().byTab.get(owner)?.rows ?? [];
      useResultStore.setState((s) =>
        setSlice(s, owner, { rows: [...prev, ...e.payload.docs] }),
      );
    }),
    listen<QueryDone>(`query:done:${streamId}`, (e) => {
      const owner = streamIdToTab.get(streamId);
      if (!owner) return;
      const slice = useResultStore.getState().byTab.get(owner);
      const baseTotal = mode === "append" ? (slice?.total ?? 0) : 0;
      const baseScanned = mode === "append" ? (slice?.scanned ?? 0) : 0;
      useResultStore.setState((s) =>
        setSlice(s, owner, {
          status: "done",
          total: baseTotal + e.payload.total,
          scanned: baseScanned + e.payload.scanned,
          tookMs: e.payload.took_ms,
          hasMore: Boolean(e.payload.has_more),
          nextCursor: e.payload.cursor ?? null,
          fetchMoreInFlight: false,
        }),
      );
      if (mode === "replace") {
        // 성공한 쿼리만 활성 프로파일 히스토리에 기록 (격리, 첫 실행만).
        const profileId = getActiveSession()?.profile_id;
        const ranDsl = useResultStore.getState().byTab.get(owner)?.lastDsl;
        if (profileId && ranDsl) {
          void useHistoryStore
            .getState()
            .record(profileId, ranDsl, e.payload.took_ms, e.payload.total);
        }
      }
      void teardownStream(streamId);
    }),
    listen<QueryErrorPayload>(`query:error:${streamId}`, (e) => {
      const owner = streamIdToTab.get(streamId);
      if (!owner) return;
      useResultStore.setState((s) =>
        setSlice(s, owner, {
          status: "error",
          error: toKoreanMessage(e.payload),
          indexUrl: e.payload.index_url ?? null,
          fetchMoreInFlight: false,
        }),
      );
      void teardownStream(streamId);
    }),
  ]);
}

export const useResultStore = create<ResultState>((set, get) => ({
  ...EMPTY_SLICE,
  byTab: new Map(),

  reset: () => {
    const tabId = useTabsStore.getState().activeTabId;
    if (!tabId) return;
    const slice = get().byTab.get(tabId);
    if (slice?.streamId) void teardownStream(slice.streamId);
    if (slice?.listenerId) {
      void stopListener(slice.listenerId).catch(() => undefined);
      void teardownListener(slice.listenerId);
    }
    set((s) => setSlice(s, tabId, { ...EMPTY_SLICE }));
  },

  runDsl: async (dsl) => {
    const tabId = useTabsStore.getState().activeTabId;
    if (!tabId) return;
    const prev = get().byTab.get(tabId);
    if (prev?.streamId) await teardownStream(prev.streamId);
    if (prev?.listenerId) {
      await stopListener(prev.listenerId).catch(() => undefined);
      await teardownListener(prev.listenerId);
    }

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
        listenerId: null,
        listenerStatus: null,
        listenerEventCount: 0,
        hasMore: false,
        nextCursor: null,
        fetchMoreInFlight: false,
      }),
    );

    // 이벤트 구독은 invoke 전에 걸어 청크 유실 방지.
    unlisteners.set(streamId, await attachStreamListeners(streamId, "replace"));

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

  selectDocument: async (path) => {
    const tabId = useTabsStore.getState().activeTabId;
    if (!tabId) return;
    // 진행 중 스트림/listener는 정리 — 단일 문서 표시 모드로 전환.
    const prev = get().byTab.get(tabId);
    if (prev?.streamId) await teardownStream(prev.streamId);
    if (prev?.listenerId) {
      await stopListener(prev.listenerId).catch(() => undefined);
      await teardownListener(prev.listenerId);
    }
    try {
      const doc = await getDocument(path);
      set((s) =>
        setSlice(s, tabId, {
          streamId: null,
          collectionPath: path,
          lastDsl: null,
          rows: doc ? [doc] : [],
          status: "done",
          total: doc ? 1 : 0,
          scanned: doc ? 1 : 0,
          tookMs: null,
          error: doc ? null : "문서를 찾을 수 없습니다.",
          indexUrl: null,
          listenerId: null,
          listenerStatus: null,
          listenerEventCount: 0,
          hasMore: false,
          nextCursor: null,
          fetchMoreInFlight: false,
        }),
      );
    } catch (err) {
      set((s) =>
        setSlice(s, tabId, {
          streamId: null,
          collectionPath: path,
          lastDsl: null,
          rows: [],
          status: "error",
          total: 0,
          scanned: 0,
          tookMs: null,
          error: toKoreanMessage(err),
          indexUrl: null,
          listenerId: null,
          listenerStatus: null,
          listenerEventCount: 0,
          hasMore: false,
          nextCursor: null,
          fetchMoreInFlight: false,
        }),
      );
    }
  },

  fetchMore: async () => {
    const tabId = useTabsStore.getState().activeTabId;
    if (!tabId) return;
    const slice = get().byTab.get(tabId);
    if (!slice) return;
    // 가드: 정상 종료 상태에서 hasMore + cursor + lastDsl이 모두 있고,
    // listener 모드가 아니며, in-flight가 아닐 때만 다음 페이지 요청.
    if (
      slice.status !== "done" ||
      !slice.hasMore ||
      !slice.nextCursor ||
      !slice.lastDsl ||
      slice.fetchMoreInFlight ||
      slice.listenerId != null
    ) {
      return;
    }

    // 기존 stream listener는 done 시점에 이미 teardownStream으로 해제됐다.
    // 백엔드 query_documents가 registry.cancel_all()을 부르므로 별도 teardown 불필요.
    const streamId = crypto.randomUUID();
    streamIdToTab.set(streamId, tabId);
    set((s) =>
      setSlice(s, tabId, {
        streamId,
        status: "streaming",
        error: null,
        indexUrl: null,
        fetchMoreInFlight: true,
        // hasMore는 done에서 갱신될 때까지 false로 두지 않고 그대로 둔다
        // — 중복 트리거는 fetchMoreInFlight 가드가 막는다.
      }),
    );

    unlisteners.set(streamId, await attachStreamListeners(streamId, "append"));

    const pagedDsl: QueryDsl = { ...slice.lastDsl, start_after: slice.nextCursor };
    try {
      await queryDocuments(streamId, pagedDsl);
    } catch (err) {
      set((s) =>
        setSlice(s, tabId, {
          status: "error",
          error: toKoreanMessage(err),
          fetchMoreInFlight: false,
        }),
      );
      await teardownStream(streamId);
    }
  },

  cancel: async () => {
    const tabId = useTabsStore.getState().activeTabId;
    if (!tabId) return;
    const sid = get().byTab.get(tabId)?.streamId;
    if (!sid) return;
    try {
      await cancelStream(sid);
    } finally {
      set((s) => setSlice(s, tabId, { status: "done", fetchMoreInFlight: false }));
      await teardownStream(sid);
    }
  },

  dropTab: (tabId) => {
    const ownedStreams: string[] = [];
    for (const [streamId, owner] of streamIdToTab.entries()) {
      if (owner === tabId) ownedStreams.push(streamId);
    }
    for (const streamId of ownedStreams) {
      void teardownStream(streamId);
    }
    const ownedListeners: string[] = [];
    for (const [listenerId, owner] of listenerIdToTab.entries()) {
      if (owner === tabId) ownedListeners.push(listenerId);
    }
    for (const listenerId of ownedListeners) {
      void stopListener(listenerId).catch(() => undefined);
      void teardownListener(listenerId);
    }
    set((s) => {
      if (!s.byTab.has(tabId)) return s;
      const map = new Map(s.byTab);
      map.delete(tabId);
      return { ...s, byTab: map };
    });
  },

  startListening: async (dsl) => {
    const tabId = useTabsStore.getState().activeTabId;
    if (!tabId) return;

    // 1) 기존 스트림/listener 정리.
    const prev = get().byTab.get(tabId);
    if (prev?.streamId) await teardownStream(prev.streamId);
    if (prev?.listenerId) {
      await stopListener(prev.listenerId).catch(() => undefined);
      await teardownListener(prev.listenerId);
    }

    // 2) 새 listener id 생성 + 슬라이스 초기화.
    const listenerId = crypto.randomUUID();
    listenerIdToTab.set(listenerId, tabId);
    const label =
      dsl.target.kind === "collection" ? dsl.target.path : `group:${dsl.target.id}`;
    set((s) =>
      setSlice(s, tabId, {
        streamId: null,
        collectionPath: label,
        lastDsl: null,
        rows: [],
        status: "listening",
        total: 0,
        scanned: 0,
        tookMs: null,
        error: null,
        indexUrl: null,
        listenerId,
        listenerStatus: "initial",
        listenerEventCount: 0,
        hasMore: false,
        nextCursor: null,
        fetchMoreInFlight: false,
      }),
    );

    // 3) 이벤트 구독 — start_listener 호출 전에 걸어 첫 스냅샷 유실 방지.
    const unlisten = await Promise.all([
      listen<ListenerChangePayload>(`listener:change:${listenerId}`, (e) => {
        const owner = listenerIdToTab.get(listenerId);
        if (!owner) return;
        const slice = get().byTab.get(owner);
        if (!slice) return;
        const { kind, doc } = e.payload;
        // path 기반 upsert / remove. path는 안정적인 키.
        let rows = slice.rows;
        if (kind === "removed") {
          rows = rows.filter((r) => r.path !== doc.path);
        } else {
          const idx = rows.findIndex((r) => r.path === doc.path);
          rows = idx >= 0
            ? rows.map((r, i) => (i === idx ? doc : r))
            : [...rows, doc];
        }
        set((s) =>
          setSlice(s, owner, {
            rows,
            total: rows.length,
            scanned: rows.length,
            listenerEventCount: slice.listenerEventCount + 1,
          }),
        );
      }),
      listen<ListenerStatusPayload>(`listener:status:${listenerId}`, (e) => {
        const owner = listenerIdToTab.get(listenerId);
        if (!owner) return;
        set((s) =>
          setSlice(s, owner, { listenerStatus: e.payload.status }),
        );
      }),
    ]);
    listenerUnlisteners.set(listenerId, unlisten);

    // 4) IPC 호출. 실패 시 슬라이스를 에러 상태로 되돌리고 unlisten.
    try {
      await startListener(listenerId, dsl);
    } catch (err) {
      await teardownListener(listenerId);
      set((s) =>
        setSlice(s, tabId, {
          status: "error",
          error: toKoreanMessage(err),
          listenerId: null,
          listenerStatus: null,
        }),
      );
    }
  },

  stopListening: async () => {
    const tabId = useTabsStore.getState().activeTabId;
    if (!tabId) return;
    const slice = get().byTab.get(tabId);
    const listenerId = slice?.listenerId;
    if (!listenerId) return;
    try {
      await stopListener(listenerId);
    } finally {
      await teardownListener(listenerId);
      set((s) =>
        setSlice(s, tabId, {
          status: "done",
          listenerId: null,
          listenerStatus: null,
        }),
      );
    }
  },

  __resetForTests: () => {
    streamIdToTab.clear();
    unlisteners.clear();
    listenerIdToTab.clear();
    listenerUnlisteners.clear();
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
