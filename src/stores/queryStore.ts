import { create } from "zustand";
import type { CompareOp } from "@/types";
import {
  buildDsl,
  EMPTY_POST_FILTER,
  type BuildResult,
  type DraftPostFilter,
  type DraftValueType,
  type DraftWhere,
  type QueryDraft,
} from "@/lib/queryDraft";
import { useTabsStore, type TabId } from "@/stores/tabsStore";

const EMPTY_WHERE: DraftWhere = {
  field: "",
  op: "==",
  valueType: "string",
  raw: "",
};

const INITIAL_DRAFT: QueryDraft = {
  targetKind: "collection",
  target: "",
  wheres: [],
  orderBys: [],
  limit: 100,
  postFilter: { ...EMPTY_POST_FILTER },
};

type QueryState = QueryDraft & {
  byTab: Map<TabId, QueryDraft>;

  setTargetKind: (k: QueryDraft["targetKind"]) => void;
  setTarget: (t: string) => void;
  addWhere: () => void;
  updateWhere: (i: number, patch: Partial<DraftWhere>) => void;
  removeWhere: (i: number) => void;
  addOrderBy: () => void;
  updateOrderBy: (
    i: number,
    patch: Partial<{ field: string; direction: "asc" | "desc" }>,
  ) => void;
  removeOrderBy: (i: number) => void;
  setLimit: (n: number) => void;
  updatePostFilter: (patch: Partial<DraftPostFilter>) => void;
  reset: () => void;
  /** 드래프트를 컬렉션 클릭 등 외부 컨텍스트로 채운다. */
  loadFromTarget: (kind: QueryDraft["targetKind"], target: string) => void;
  /** 히스토리 등에서 받은 완성 드래프트로 교체. */
  hydrate: (d: QueryDraft) => void;
  build: () => BuildResult;

  __resetForTests: () => void;
};

function setSlice(
  state: QueryState,
  tabId: TabId,
  patch: Partial<QueryDraft>,
): QueryState {
  const prev = state.byTab.get(tabId) ?? INITIAL_DRAFT;
  const next: QueryDraft = { ...prev, ...patch };
  const map = new Map(state.byTab);
  map.set(tabId, next);
  const isActive = useTabsStore.getState().activeTabId === tabId;
  return isActive
    ? { ...state, ...next, byTab: map }
    : { ...state, byTab: map };
}

function activeMutate(
  state: QueryState,
  fn: (draft: QueryDraft) => Partial<QueryDraft>,
): QueryState {
  const tabId = useTabsStore.getState().activeTabId;
  if (!tabId) return state;
  const prev = state.byTab.get(tabId) ?? INITIAL_DRAFT;
  return setSlice(state, tabId, fn(prev));
}

export const useQueryStore = create<QueryState>((set, get) => ({
  ...INITIAL_DRAFT,
  byTab: new Map(),

  setTargetKind: (targetKind) => set((s) => activeMutate(s, () => ({ targetKind }))),
  setTarget: (target) => set((s) => activeMutate(s, () => ({ target }))),

  addWhere: () =>
    set((s) =>
      activeMutate(s, (d) => ({ wheres: [...d.wheres, { ...EMPTY_WHERE }] })),
    ),
  updateWhere: (i, patch) =>
    set((s) =>
      activeMutate(s, (d) => ({
        wheres: d.wheres.map((w, idx) => (idx === i ? { ...w, ...patch } : w)),
      })),
    ),
  removeWhere: (i) =>
    set((s) =>
      activeMutate(s, (d) => ({
        wheres: d.wheres.filter((_, idx) => idx !== i),
      })),
    ),

  addOrderBy: () =>
    set((s) =>
      activeMutate(s, (d) => ({
        orderBys: [...d.orderBys, { field: "", direction: "asc" }],
      })),
    ),
  updateOrderBy: (i, patch) =>
    set((s) =>
      activeMutate(s, (d) => ({
        orderBys: d.orderBys.map((o, idx) =>
          idx === i ? { ...o, ...patch } : o,
        ),
      })),
    ),
  removeOrderBy: (i) =>
    set((s) =>
      activeMutate(s, (d) => ({
        orderBys: d.orderBys.filter((_, idx) => idx !== i),
      })),
    ),

  setLimit: (limit) =>
    set((s) => activeMutate(s, () => ({ limit: Number.isFinite(limit) ? limit : 0 }))),

  updatePostFilter: (patch) =>
    set((s) =>
      activeMutate(s, (d) => ({ postFilter: { ...d.postFilter, ...patch } })),
    ),

  reset: () =>
    set((s) =>
      activeMutate(s, () => ({
        ...INITIAL_DRAFT,
        postFilter: { ...EMPTY_POST_FILTER },
      })),
    ),

  loadFromTarget: (targetKind, target) =>
    set((s) =>
      activeMutate(s, () => ({
        targetKind,
        target,
        wheres: [],
        orderBys: [],
        limit: 100,
        postFilter: { ...EMPTY_POST_FILTER },
      })),
    ),

  hydrate: (d) =>
    set((s) =>
      activeMutate(s, () => ({
        targetKind: d.targetKind,
        target: d.target,
        wheres: d.wheres.map((w) => ({ ...w })),
        orderBys: d.orderBys.map((o) => ({ ...o })),
        limit: d.limit,
        postFilter: { ...d.postFilter },
      })),
    ),

  build: () => {
    const s = get();
    return buildDsl({
      targetKind: s.targetKind,
      target: s.target,
      wheres: s.wheres,
      orderBys: s.orderBys,
      limit: s.limit,
      postFilter: s.postFilter,
    });
  },

  __resetForTests: () => set({ ...INITIAL_DRAFT, byTab: new Map() }),
}));

// 활성 탭 변경 시 top-level 미러 동기화
useTabsStore.subscribe((s, prev) => {
  if (s.activeTabId === prev.activeTabId) return;
  const draft = s.activeTabId
    ? useQueryStore.getState().byTab.get(s.activeTabId) ?? INITIAL_DRAFT
    : INITIAL_DRAFT;
  useQueryStore.setState(draft);
});

export const COMPARE_OPS: CompareOp[] = [
  "==", "!=", "<", "<=", ">", ">=",
  "array_contains", "array_contains_any", "in", "not_in",
];

export const VALUE_TYPES: DraftValueType[] = [
  "string", "int", "double", "bool", "null", "timestamp", "reference",
];
