import { useMemo, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "sonner";
import { useLogStore } from "@/stores/logStore";
import { useSessionStore } from "@/stores/sessionStore";
import { cn } from "@/lib/utils";
import { type LogEntry, type LogLevel } from "@/types";
import { toKoreanMessage } from "@/lib/errorMessages";

const LEVELS: LogLevel[] = ["error", "warn", "info", "debug"];
const COLOR: Record<LogLevel, string> = {
  error: "text-destructive",
  warn: "text-amber-600",
  info: "text-foreground",
  debug: "text-muted-foreground",
};

export function LogView() {
  const entries = useLogStore((s) => s.entries);
  const levels = useLogStore((s) => s.levels);
  const onlyActive = useLogStore((s) => s.onlyActiveProfile);
  const toggleLevel = useLogStore((s) => s.toggleLevel);
  const setOnly = useLogStore((s) => s.setOnlyActiveProfile);
  const clear = useLogStore((s) => s.clear);
  const activeId = useSessionStore((s) => s.current?.profile_id ?? null);

  const filtered = useMemo(
    () =>
      entries.filter(
        (e) =>
          levels.includes(e.level) &&
          (!onlyActive || !activeId || e.profile_id === activeId),
      ),
    [entries, levels, onlyActive, activeId],
  );

  const parentRef = useRef<HTMLDivElement>(null);
  const v = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 22,
    overscan: 20,
  });

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 border-b px-3 py-1.5 text-xs">
        {LEVELS.map((l) => (
          <button
            key={l}
            type="button"
            onClick={() => toggleLevel(l)}
            aria-pressed={levels.includes(l)}
            className={cn(
              "rounded px-1.5 py-0.5 font-medium",
              levels.includes(l) ? "bg-accent" : "text-muted-foreground/50",
            )}
          >
            {l}
          </button>
        ))}
        <label className="ml-2 flex items-center gap-1 text-muted-foreground">
          <input
            type="checkbox"
            checked={onlyActive}
            onChange={(e) => setOnly(e.target.checked)}
          />
          활성 프로파일만
        </label>
        <button
          type="button"
          onClick={() => void copyVisible(filtered)}
          disabled={filtered.length === 0}
          className="ml-auto rounded px-1.5 py-0.5 text-muted-foreground hover:bg-accent disabled:opacity-50"
          title="화면에 보이는 로그를 텍스트로 클립보드 복사"
        >
          복사 ({filtered.length})
        </button>
        <button
          type="button"
          onClick={clear}
          className="rounded px-1.5 py-0.5 text-muted-foreground hover:bg-accent"
        >
          지우기
        </button>
      </div>
      <div ref={parentRef} className="flex-1 overflow-auto font-mono text-xs">
        <div style={{ height: v.getTotalSize(), position: "relative" }}>
          {v.getVirtualItems().map((vi) => {
            const e = filtered[vi.index];
            return (
              <div
                key={vi.key}
                className="absolute inset-x-0 flex gap-2 px-3 py-0.5"
                style={{ height: 22, transform: `translateY(${vi.start}px)` }}
              >
                <span className="shrink-0 text-muted-foreground">
                  {e.ts.slice(11, 19)}
                </span>
                <span className={cn("w-12 shrink-0 uppercase", COLOR[e.level])}>
                  {e.level}
                </span>
                <span className="truncate" title={e.message}>
                  {e.message}
                </span>
              </div>
            );
          })}
        </div>
        {filtered.length === 0 && (
          <div className="flex h-full items-center justify-center text-muted-foreground">
            로그 없음
          </div>
        )}
      </div>
    </div>
  );
}

/** 화면에 보이는 로그를 사람이 읽기 좋은 텍스트로 클립보드에 복사. */
async function copyVisible(entries: LogEntry[]): Promise<void> {
  if (entries.length === 0) return;
  const text = entries.map(formatLogLine).join("\n");
  try {
    await writeText(text);
    toast.success(`${entries.length.toLocaleString()}개 로그 복사`);
  } catch (err) {
    toast.error(toKoreanMessage(err));
  }
}

/** LogView가 화면에 표시하는 컬럼 + 메시지를 한 줄로 직렬화. */
export function formatLogLine(e: LogEntry): string {
  const time = e.ts.slice(11, 19);
  return `${time} ${e.level.toUpperCase().padEnd(5)} ${e.message}`;
}
