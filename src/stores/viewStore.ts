import { create } from "zustand";
import { useTabsStore, type TabId } from "@/stores/tabsStore";

export type ViewKind = "table" | "tree" | "json" | "log";

type ViewSlice = { activeView: ViewKind };

const INITIAL_SLICE: ViewSlice = { activeView: "table" };

type ViewState = ViewSlice & {
  byTab: Map<TabId, ViewSlice>;
  setView: (v: ViewKind) => void;
  __resetForTests: () => void;
};

function setSlice(
  state: ViewState,
  tabId: TabId,
  patch: Partial<ViewSlice>,
): ViewState {
  const prev = state.byTab.get(tabId) ?? INITIAL_SLICE;
  const next: ViewSlice = { ...prev, ...patch };
  const map = new Map(state.byTab);
  map.set(tabId, next);
  const isActive = useTabsStore.getState().activeTabId === tabId;
  return isActive
    ? { ...state, ...next, byTab: map }
    : { ...state, byTab: map };
}

export const useViewStore = create<ViewState>((set) => ({
  ...INITIAL_SLICE,
  byTab: new Map(),

  setView: (activeView) =>
    set((s) => {
      const tabId = useTabsStore.getState().activeTabId;
      if (!tabId) return s;
      return setSlice(s, tabId, { activeView });
    }),

  __resetForTests: () => set({ ...INITIAL_SLICE, byTab: new Map() }),
}));

useTabsStore.subscribe((s, prev) => {
  if (s.activeTabId === prev.activeTabId) return;
  const slice = s.activeTabId
    ? useViewStore.getState().byTab.get(s.activeTabId) ?? INITIAL_SLICE
    : INITIAL_SLICE;
  useViewStore.setState(slice);
});
