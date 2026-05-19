import { SlidersHorizontal } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ModeIcon } from "@/components/profile/mode";
import { useResultStore } from "@/stores/resultStore";
import { ViewTabs } from "./ViewTabs";
import type { ProfileMode } from "@/types";

export function ResultBar({
  projectId,
  mode,
  builderOpen,
  onToggleBuilder,
}: {
  projectId: string;
  mode: ProfileMode;
  builderOpen: boolean;
  onToggleBuilder: () => void;
}) {
  const path = useResultStore((s) => s.collectionPath);
  const status = useResultStore((s) => s.status);
  const rows = useResultStore((s) => s.rows.length);
  const total = useResultStore((s) => s.total);
  const scanned = useResultStore((s) => s.scanned);
  const tookMs = useResultStore((s) => s.tookMs);
  const error = useResultStore((s) => s.error);
  const cancel = useResultStore((s) => s.cancel);

  return (
    <div className="flex items-center gap-3 border-b px-3 py-1.5 text-xs">
      <Button
        type="button"
        size="sm"
        variant={builderOpen ? "secondary" : "ghost"}
        className="h-7 gap-1.5 px-2 text-xs"
        aria-pressed={builderOpen}
        onClick={onToggleBuilder}
      >
        <SlidersHorizontal className="size-3.5" />
        쿼리 빌더
      </Button>
      <span className="h-4 w-px bg-border" />
      <ViewTabs />
      <span className="h-4 w-px bg-border" />
      <ModeIcon mode={mode} className="size-4 text-muted-foreground" />
      <span className="text-muted-foreground">{projectId}</span>
      {path && (
        <span className="font-medium">
          / {path}
        </span>
      )}
      <span className="ml-auto flex items-center gap-3">
        {status === "streaming" && (
          <>
            <span className="text-muted-foreground">
              스트리밍… {rows}건
            </span>
            <Button size="sm" variant="ghost" onClick={() => void cancel()}>
              취소
            </Button>
          </>
        )}
        {status === "done" && (
          <span className="text-muted-foreground">
            {scanned > total
              ? `전체 ${scanned}건 중 ${total}건 매칭`
              : `${total}건`}
            {tookMs != null ? ` · ${tookMs}ms` : ""}
          </span>
        )}
        {status === "error" && (
          <span className="text-destructive">{error}</span>
        )}
      </span>
    </div>
  );
}
