import { create } from "zustand";
import type { Session } from "@/types";

// 현재 활성 세션. 백엔드 `profile:activated/deactivated` 이벤트로 동기화.
type SessionState = {
  current: Session | null;
  setCurrent: (s: Session | null) => void;
};

export const useSessionStore = create<SessionState>((set) => ({
  current: null,
  setCurrent: (s) => set({ current: s }),
}));
