import { create } from "zustand";
import {
  addQueryHistory,
  clearQueryHistory,
  listQueryHistory,
  removeQueryHistory,
} from "@/ipc/query";
import { asAppError, type QueryDsl, type QueryHistoryEntry } from "@/types";

// 활성 프로파일의 쿼리 히스토리 (백엔드가 영속화·디듀프·캡 담당,
// `docs/03-ipc-contract.md` §8). 스토어는 백엔드 상태의 미러일 뿐.
type HistoryState = {
  profileId: string | null;
  entries: QueryHistoryEntry[];
  loading: boolean;
  /** 프로파일 전환 시 호출 — 해당 프로파일 히스토리 로드. */
  load: (profileId: string | null) => Promise<void>;
  /** 쿼리 성공 후 기록. 백엔드 디듀프·캡 반영 위해 재조회. */
  record: (
    profileId: string,
    dsl: QueryDsl,
    tookMs: number | null,
    resultCount: number | null,
  ) => Promise<void>;
  remove: (entryId: string) => Promise<void>;
  clear: () => Promise<void>;
};

export const useHistoryStore = create<HistoryState>((set, get) => ({
  profileId: null,
  entries: [],
  loading: false,

  load: async (profileId) => {
    set({ profileId, entries: [], loading: !!profileId });
    if (!profileId) return;
    try {
      const entries = await listQueryHistory(profileId);
      // 늦게 도착한 응답이 다른 프로파일을 덮어쓰지 않도록 가드.
      if (get().profileId === profileId) set({ entries, loading: false });
    } catch {
      if (get().profileId === profileId) set({ loading: false });
    }
  },

  record: async (profileId, dsl, tookMs, resultCount) => {
    try {
      await addQueryHistory({
        profile_id: profileId,
        dsl,
        took_ms: tookMs ?? undefined,
        result_count: resultCount ?? undefined,
      });
      if (get().profileId === profileId) {
        set({ entries: await listQueryHistory(profileId) });
      }
    } catch {
      // 히스토리 기록 실패는 쿼리 결과에 영향 주지 않음 — 조용히 무시.
    }
  },

  remove: async (entryId) => {
    const pid = get().profileId;
    if (!pid) return;
    try {
      await removeQueryHistory(pid, entryId);
      set((s) => ({ entries: s.entries.filter((e) => e.id !== entryId) }));
    } catch (err) {
      throw asAppError(err);
    }
  },

  clear: async () => {
    const pid = get().profileId;
    if (!pid) return;
    try {
      await clearQueryHistory(pid);
      set({ entries: [] });
    } catch (err) {
      throw asAppError(err);
    }
  },
}));
