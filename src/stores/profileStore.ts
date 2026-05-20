import { create } from "zustand";
import { listProfiles } from "@/ipc/profile";
import { type ProfileMeta } from "@/types";
import { toKoreanMessage } from "@/lib/errorMessages";

// 원칙 10·13: `profiles`는 백엔드에서 받은 그대로(단일 진실 원천),
// 나머지는 파생/UI 상태. 갱신은 `profile:*` 이벤트로 자동 동기화한다.
type ProfileState = {
  profiles: ProfileMeta[];
  loading: boolean;
  error: string | null;
  load: () => Promise<void>;
  upsert: (meta: ProfileMeta) => void;
  removeById: (id: string) => void;
};

export const useProfileStore = create<ProfileState>((set) => ({
  profiles: [],
  loading: false,
  error: null,

  load: async () => {
    set({ loading: true, error: null });
    try {
      set({ profiles: await listProfiles(), loading: false });
    } catch (err) {
      set({ loading: false, error: toKoreanMessage(err) });
    }
  },

  upsert: (meta) =>
    set((s) => {
      const idx = s.profiles.findIndex((p) => p.id === meta.id);
      if (idx === -1) return { profiles: [...s.profiles, meta] };
      const next = s.profiles.slice();
      next[idx] = meta;
      return { profiles: next };
    }),

  removeById: (id) =>
    set((s) => ({ profiles: s.profiles.filter((p) => p.id !== id) })),
}));
