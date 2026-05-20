import { create } from "zustand";
import type { Session } from "@/types";

export type TabId = string;

export type Tab = {
  id: TabId;
  session: Session | null;
  /**
   * 휴면 상태(spec §5): 어떤 프로파일에 속하는 탭인지는 알지만 아직 활성화는 안 함.
   * PR 4 hydrate에서 운영 프로파일에 사용. PR 2에서는 항상 undefined.
   */
  pendingProfileId?: string;
  label?: string;
};

type TabsState = {
  tabs: Tab[];
  activeTabId: TabId | null;

  add: () => TabId;
  close: (id: TabId) => void;
  focus: (id: TabId) => void;
  setSession: (id: TabId, session: Session | null) => void;
  setPendingProfileId: (id: TabId, profileId: string | undefined) => void;
  activeSessionId: () => string | null;
  __resetForTests: () => void;
};

function newTab(): Tab {
  return { id: crypto.randomUUID(), session: null };
}

function initialState(): Pick<TabsState, "tabs" | "activeTabId"> {
  const first = newTab();
  return { tabs: [first], activeTabId: first.id };
}

export const useTabsStore = create<TabsState>((set, get) => ({
  ...initialState(),

  add: () => {
    const t = newTab();
    set((s) => ({ tabs: [...s.tabs, t], activeTabId: t.id }));
    return t.id;
  },

  close: (id) => {
    for (const fn of closeCleanups) {
      try {
        fn(id);
      } catch (err) {
        // 콜백 실패는 탭 종료 자체를 막지 않는다. 로그만 남김.
        // eslint-disable-next-line no-console
        console.error("tab close cleanup failed", err);
      }
    }
    set((s) => {
      const idx = s.tabs.findIndex((t) => t.id === id);
      if (idx < 0) return s;
      const next = s.tabs.filter((t) => t.id !== id);
      if (next.length === 0) {
        const t = newTab();
        return { tabs: [t], activeTabId: t.id };
      }
      let newActive = s.activeTabId;
      if (s.activeTabId === id) {
        const fallback = next[Math.min(idx, next.length - 1)];
        newActive = fallback.id;
      }
      return { tabs: next, activeTabId: newActive };
    });
  },

  focus: (id) => {
    set((s) => (s.tabs.some((t) => t.id === id) ? { activeTabId: id } : s));
  },

  setSession: (id, session) => {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id
          ? { ...t, session, pendingProfileId: undefined }
          : t,
      ),
    }));
  },

  setPendingProfileId: (id, profileId) => {
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === id ? { ...t, pendingProfileId: profileId } : t,
      ),
    }));
  },

  activeSessionId: () => {
    const s = get();
    return s.tabs.find((t) => t.id === s.activeTabId)?.session?.session_id ?? null;
  },

  __resetForTests: () => set(initialState()),
}));

/** 활성 탭 ID 셀렉터 (Zustand hook). PR 3 컴포넌트가 사용. */
export function useActiveTabId(): TabId | null {
  return useTabsStore((s) => s.activeTabId);
}

/** 활성 탭 세션 셀렉터 (Zustand hook). PR 2에서 useSessionStore를 대체. */
export function useActiveSession(): Session | null {
  return useTabsStore(
    (s) => s.tabs.find((t) => t.id === s.activeTabId)?.session ?? null,
  );
}

/** Imperative read of the active session (for use outside React render). */
export function getActiveSession(): Session | null {
  const s = useTabsStore.getState();
  return s.tabs.find((t) => t.id === s.activeTabId)?.session ?? null;
}

/** Imperative read of the active session_id. */
export function getActiveSessionId(): string | null {
  return getActiveSession()?.session_id ?? null;
}

type CloseCleanup = (tabId: TabId) => void;
const closeCleanups: CloseCleanup[] = [];

export function registerTabCloseCleanup(fn: CloseCleanup): void {
  closeCleanups.push(fn);
}
