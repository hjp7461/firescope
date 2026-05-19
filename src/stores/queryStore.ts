import { create } from "zustand";
import type { CompareOp } from "@/types";
import {
  buildDsl,
  type BuildResult,
  type DraftValueType,
  type DraftWhere,
  type QueryDraft,
} from "@/lib/queryDraft";

type QueryState = QueryDraft & {
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
  reset: () => void;
  /** 드래프트를 컬렉션 클릭 등 외부 컨텍스트로 채운다. */
  loadFromTarget: (kind: QueryDraft["targetKind"], target: string) => void;
  /** 히스토리 등에서 받은 완성 드래프트로 교체. */
  hydrate: (d: QueryDraft) => void;
  build: () => BuildResult;
};

const EMPTY_WHERE: DraftWhere = {
  field: "",
  op: "==",
  valueType: "string",
  raw: "",
};

const initial: QueryDraft = {
  targetKind: "collection",
  target: "",
  wheres: [],
  orderBys: [],
  limit: 100,
};

export const useQueryStore = create<QueryState>((set, get) => ({
  ...initial,

  setTargetKind: (targetKind) => set({ targetKind }),
  setTarget: (target) => set({ target }),

  addWhere: () =>
    set((s) => ({ wheres: [...s.wheres, { ...EMPTY_WHERE }] })),
  updateWhere: (i, patch) =>
    set((s) => ({
      wheres: s.wheres.map((w, idx) => (idx === i ? { ...w, ...patch } : w)),
    })),
  removeWhere: (i) =>
    set((s) => ({ wheres: s.wheres.filter((_, idx) => idx !== i) })),

  addOrderBy: () =>
    set((s) => ({
      orderBys: [...s.orderBys, { field: "", direction: "asc" }],
    })),
  updateOrderBy: (i, patch) =>
    set((s) => ({
      orderBys: s.orderBys.map((o, idx) =>
        idx === i ? { ...o, ...patch } : o,
      ),
    })),
  removeOrderBy: (i) =>
    set((s) => ({ orderBys: s.orderBys.filter((_, idx) => idx !== i) })),

  setLimit: (limit) => set({ limit: Number.isFinite(limit) ? limit : 0 }),

  reset: () => set({ ...initial }),

  loadFromTarget: (targetKind, target) =>
    set({ targetKind, target, wheres: [], orderBys: [], limit: 100 }),

  hydrate: (d) =>
    set({
      targetKind: d.targetKind,
      target: d.target,
      wheres: d.wheres.map((w) => ({ ...w })),
      orderBys: d.orderBys.map((o) => ({ ...o })),
      limit: d.limit,
    }),

  build: () => {
    const s = get();
    return buildDsl({
      targetKind: s.targetKind,
      target: s.target,
      wheres: s.wheres,
      orderBys: s.orderBys,
      limit: s.limit,
    });
  },
}));

export const COMPARE_OPS: CompareOp[] = [
  "==",
  "!=",
  "<",
  "<=",
  ">",
  ">=",
  "array_contains",
  "array_contains_any",
  "in",
  "not_in",
];

export const VALUE_TYPES: DraftValueType[] = [
  "string",
  "int",
  "double",
  "bool",
  "null",
  "timestamp",
  "reference",
];
