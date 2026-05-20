import type * as React from "react";
import { Clock, Star, Trash2, X } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { useHistoryStore } from "@/stores/historyStore";
import { useResultStore } from "@/stores/resultStore";
import { type QueryDsl, type QueryHistoryEntry } from "@/types";
import { toKoreanMessage } from "@/lib/errorMessages";

function targetLabel(dsl: QueryDsl): string {
  return dsl.target.kind === "collection"
    ? dsl.target.path
    : `group:${dsl.target.id}`;
}

function summarize(dsl: QueryDsl): string {
  const parts: string[] = [];
  if (dsl.where?.length) parts.push(`where ${dsl.where.length}`);
  if (dsl.order_by?.length) parts.push(`order ${dsl.order_by.length}`);
  if (dsl.limit) parts.push(`limit ${dsl.limit}`);
  if (dsl.post_filter) parts.push("후처리");
  return parts.join(" · ") || "조건 없음";
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="px-2 pt-1.5 pb-0.5 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
      {children}
    </div>
  );
}

function HistoryItem({
  entry,
  onReplay,
  onRemove,
  onTogglePin,
}: {
  entry: QueryHistoryEntry;
  onReplay: () => void;
  onRemove: () => void;
  onTogglePin: () => void;
}) {
  const when = new Date(entry.executed_at).toLocaleString();
  const meta = [
    entry.result_count != null ? `${entry.result_count}건` : null,
    entry.took_ms != null ? `${entry.took_ms}ms` : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <div className="group flex items-start gap-1.5 rounded-md px-2 py-1.5 hover:bg-accent/50">
      <Button
        type="button"
        size="icon"
        variant="ghost"
        className="size-6 shrink-0"
        onClick={onTogglePin}
        aria-label={entry.pinned ? "핀 해제" : "핀 고정"}
        title={entry.pinned ? "핀 해제" : "핀 고정 (100개 캡에서 제외)"}
      >
        <Star
          className={cn(
            "size-3.5",
            entry.pinned
              ? "fill-amber-400 text-amber-500"
              : "text-muted-foreground/40",
          )}
        />
      </Button>
      <button
        type="button"
        onClick={onReplay}
        className="flex min-w-0 flex-1 flex-col items-start gap-0.5 text-left"
        title="이 쿼리를 다시 실행"
      >
        <span className="w-full truncate text-xs font-medium">
          {targetLabel(entry.dsl)}
        </span>
        <span className="w-full truncate text-[11px] text-muted-foreground">
          {summarize(entry.dsl)}
        </span>
        <span className="text-[10px] text-muted-foreground">
          {when}
          {meta ? ` · ${meta}` : ""}
        </span>
      </button>
      <Button
        type="button"
        size="icon"
        variant="ghost"
        className="size-6 shrink-0 opacity-0 group-hover:opacity-100"
        onClick={onRemove}
        aria-label="이 히스토리 삭제"
      >
        <X className="size-3.5" />
      </Button>
    </div>
  );
}

export function HistoryPanel() {
  const entries = useHistoryStore((s) => s.entries);
  const loading = useHistoryStore((s) => s.loading);
  const profileId = useHistoryStore((s) => s.profileId);
  const remove = useHistoryStore((s) => s.remove);
  const clear = useHistoryStore((s) => s.clear);
  const togglePin = useHistoryStore((s) => s.togglePin);
  const runDsl = useResultStore((s) => s.runDsl);

  const pinned = entries.filter((e) => e.pinned);
  const recent = entries.filter((e) => !e.pinned);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex items-center justify-between border-b px-2 py-1.5">
        <span className="flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
          <Clock className="size-3" />
          히스토리
        </span>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          className="h-6 gap-1 px-1.5 text-xs"
          disabled={entries.length === 0}
          onClick={() => {
            void clear().catch((e) => toast.error(toKoreanMessage(e)));
          }}
        >
          <Trash2 className="size-3" />
          전체 삭제
        </Button>
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <div className={cn("flex flex-col gap-0.5 p-1.5")}>
          {!profileId ? (
            <p className="px-2 py-4 text-xs text-muted-foreground">
              활성 프로파일 없음
            </p>
          ) : loading ? (
            <p className="px-2 py-4 text-xs text-muted-foreground">
              불러오는 중…
            </p>
          ) : entries.length === 0 ? (
            <p className="px-2 py-4 text-xs text-muted-foreground">
              아직 실행한 쿼리가 없습니다
            </p>
          ) : (
            <>
              {pinned.length > 0 && (
                <>
                  <SectionLabel>북마크 ({pinned.length})</SectionLabel>
                  {pinned.map((e) => (
                    <HistoryItem
                      key={e.id}
                      entry={e}
                      onReplay={() => void runDsl(e.dsl)}
                      onRemove={() => {
                        void remove(e.id).catch((err) =>
                          toast.error(toKoreanMessage(err)),
                        );
                      }}
                      onTogglePin={() => {
                        void togglePin(e.id, !e.pinned).catch((err) =>
                          toast.error(toKoreanMessage(err)),
                        );
                      }}
                    />
                  ))}
                  <SectionLabel>최근 ({recent.length})</SectionLabel>
                </>
              )}
              {recent.map((e) => (
                <HistoryItem
                  key={e.id}
                  entry={e}
                  onReplay={() => void runDsl(e.dsl)}
                  onRemove={() => {
                    void remove(e.id).catch((err) =>
                      toast.error(toKoreanMessage(err)),
                    );
                  }}
                  onTogglePin={() => {
                    void togglePin(e.id, !e.pinned).catch((err) =>
                      toast.error(toKoreanMessage(err)),
                    );
                  }}
                />
              ))}
            </>
          )}
        </div>
      </ScrollArea>
    </div>
  );
}
