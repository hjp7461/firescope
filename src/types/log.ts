// 03-ipc-contract.md §6 `log:entry` 페이로드와 1:1.
export type LogLevel = "error" | "warn" | "info" | "debug";

export type LogEntry = {
  level: LogLevel;
  message: string;
  target: string;
  ts: string;
  profile_id?: string;
};

const LEVELS: LogLevel[] = ["error", "warn", "info", "debug"];

/** 미상 레벨은 info로 강등 (드롭 금지 — 관찰가능성). */
export function normalizeLevel(x: unknown): LogLevel {
  return LEVELS.includes(x as LogLevel) ? (x as LogLevel) : "info";
}
