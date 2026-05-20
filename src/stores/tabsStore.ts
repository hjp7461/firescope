import { create } from "zustand";
import type { ProfileMeta, Session, TabBundle, TabRecord } from "@/types";

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

// --- Persistence ---

let persistenceEnabled = false;

/** PR 4: 하이드레이트 도중에는 false로 두고, 완료 후 true. save subscriber가 이를 검사. */
export function setPersistenceEnabled(enabled: boolean): void {
  persistenceEnabled = enabled;
}

/** 현재 store 상태를 TabBundle로 직렬화. profile_id는 active session 우선, 없으면 pendingProfileId. */
export function tabsToBundle(): TabBundle {
  const s = useTabsStore.getState();
  return {
    version: 1,
    tabs: s.tabs.map((tab, order): TabRecord => {
      const profile_id = tab.session?.profile_id ?? tab.pendingProfileId;
      const record: TabRecord = { id: tab.id, order };
      if (profile_id) record.profile_id = profile_id;
      return record;
    }),
    active_tab_id: s.activeTabId ?? undefined,
  };
}

/**
 * 저장된 TabBundle에서 탭 상태 복원.
 *
 * - 빈 bundle: 기본 상태(빈 탭 1개) 유지
 * - 일반 프로파일: `activate(profile_id)` 호출 → setSession
 * - 운영 프로파일(`require_confirmation=true`): `pendingProfileId`만 설정(휴면)
 * - 프로파일 삭제됨/누락: 빈 탭으로 두고 무시
 * - 활성화 실패(자격증명 없음 등): catch 후 빈 탭 유지
 */
export async function hydrateTabs(
  bundle: TabBundle,
  profilesById: Map<string, ProfileMeta>,
  activate: (profile_id: string) => Promise<Session>,
): Promise<void> {
  if (bundle.tabs.length === 0) return;

  // 1) 탭 골격 설치
  const restored: Tab[] = bundle.tabs.map((rec) => ({
    id: rec.id,
    session: null,
  }));
  const validActive =
    bundle.active_tab_id && restored.some((t) => t.id === bundle.active_tab_id)
      ? bundle.active_tab_id
      : restored[0]?.id ?? null;
  useTabsStore.setState({ tabs: restored, activeTabId: validActive });

  // 2) 각 탭의 프로파일 결정
  for (const rec of bundle.tabs) {
    if (!rec.profile_id) continue;
    const profile = profilesById.get(rec.profile_id);
    if (!profile) continue;

    if (profile.require_confirmation) {
      useTabsStore.getState().setPendingProfileId(rec.id, rec.profile_id);
    } else {
      try {
        const session = await activate(rec.profile_id);
        useTabsStore.getState().setSession(rec.id, session);
      } catch {
        // 빈 탭 유지
      }
    }
  }
}

type CloseCleanup = (tabId: TabId) => void;
const closeCleanups: CloseCleanup[] = [];

export function registerTabCloseCleanup(fn: CloseCleanup): void {
  closeCleanups.push(fn);
}

// --- Debounced persistence subscribe ---

let saveTimer: ReturnType<typeof setTimeout> | null = null;
const SAVE_DEBOUNCE_MS = 200;

/**
 * 모듈 레벨 subscribe. `persistenceEnabled`가 true일 때만 디바운스 save 발사.
 * 하이드레이트 동안에는 false로 두어 partial state 저장을 방지.
 */
useTabsStore.subscribe((state, prev) => {
  if (!persistenceEnabled) return;
  if (state.tabs === prev.tabs && state.activeTabId === prev.activeTabId) return;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    import("@/ipc/tabs")
      .then(({ saveTabs }) => saveTabs(tabsToBundle()))
      .catch(() => {
        // 저장 실패는 사용자 흐름을 막지 않는다.
      });
  }, SAVE_DEBOUNCE_MS);
});
