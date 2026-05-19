import { Button } from "@/components/ui/button";
import { ModeIcon } from "@/components/profile/mode";
import { useResultStore } from "@/stores/resultStore";
import type { ProfileMode } from "@/types";

export function ResultBar({
  projectId,
  mode,
}: {
  projectId: string;
  mode: ProfileMode;
}) {
  const path = useResultStore((s) => s.collectionPath);
  const status = useResultStore((s) => s.status);
  const rows = useResultStore((s) => s.rows.length);
  const total = useResultStore((s) => s.total);
  const tookMs = useResultStore((s) => s.tookMs);
  const error = useResultStore((s) => s.error);
  const cancel = useResultStore((s) => s.cancel);

  return (
    <div className="flex items-center gap-3 border-b px-3 py-1.5 text-xs">
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
            {total}건{tookMs != null ? ` · ${tookMs}ms` : ""}
          </span>
        )}
        {status === "error" && (
          <span className="text-destructive">{error}</span>
        )}
      </span>
    </div>
  );
}
