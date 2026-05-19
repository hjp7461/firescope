import { Clock, Trash2, X } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { useHistoryStore } from "@/stores/historyStore";
import { useResultStore } from "@/stores/resultStore";
import { asAppError, type QueryDsl, type QueryHistoryEntry } from "@/types";

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

function HistoryItem({
  entry,
  onReplay,
  onRemove,
}: {
  entry: QueryHistoryEntry;
  onReplay: () => void;
  onRemove: () => void;
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
  const runDsl = useResultStore((s) => s.runDsl);

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
            void clear().catch((e) => toast.error(asAppError(e).message));
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
            entries.map((e) => (
              <HistoryItem
                key={e.id}
                entry={e}
                onReplay={() => void runDsl(e.dsl)}
                onRemove={() => {
                  void remove(e.id).catch((err) =>
                    toast.error(asAppError(err).message),
                  );
                }}
              />
            ))
          )}
        </div>
      </ScrollArea>
    </div>
  );
}
