import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { appendBounded } from "@/lib/ringBuffer";
import { normalizeLevel, type LogEntry, type LogLevel } from "@/types";

const MAX = 2000;
const ALL: LogLevel[] = ["error", "warn", "info", "debug"];

type LogState = {
  entries: LogEntry[];
  levels: LogLevel[];
  onlyActiveProfile: boolean;
  clear: () => void;
  toggleLevel: (l: LogLevel) => void;
  setOnlyActiveProfile: (v: boolean) => void;
};

export const useLogStore = create<LogState>((set) => ({
  entries: [],
  levels: ALL,
  onlyActiveProfile: false,
  clear: () => set({ entries: [] }),
  toggleLevel: (l) =>
    set((s) => ({
      levels: s.levels.includes(l)
        ? s.levels.filter((x) => x !== l)
        : [...s.levels, l],
    })),
  setOnlyActiveProfile: (onlyActiveProfile) => set({ onlyActiveProfile }),
}));

// 모듈 스코프 1회 구독 + rAF 배치 (resultStore 패턴, 백프레셔).
let pending: LogEntry[] = [];
let raf = 0;
function flush() {
  raf = 0;
  const batch = pending;
  pending = [];
  useLogStore.setState((s) => {
    let next = s.entries;
    for (const e of batch) next = appendBounded(next, e, MAX);
    return { entries: next };
  });
}
let unlisten: UnlistenFn | null = null;
export async function startLogStream() {
  if (unlisten) return;
  unlisten = await listen<LogEntry>("log:entry", (e) => {
    pending.push({ ...e.payload, level: normalizeLevel(e.payload.level) });
    if (!raf) raf = requestAnimationFrame(flush);
  });
}
