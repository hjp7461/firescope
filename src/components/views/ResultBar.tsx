import { useState } from "react";
import { Calculator, ClipboardCopy, Download, SlidersHorizontal } from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ModeIcon } from "@/components/profile/mode";
import { useResultStore } from "@/stores/resultStore";
import { useViewStore } from "@/stores/viewStore";
import { exportResult, queryCount } from "@/ipc/query";
import { asAppError, type ExportFormat, type ExportSource, type ProfileMode } from "@/types";
import { ViewTabs } from "./ViewTabs";

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
  const rowsData = useResultStore((s) => s.rows);
  const total = useResultStore((s) => s.total);
  const scanned = useResultStore((s) => s.scanned);
  const tookMs = useResultStore((s) => s.tookMs);
  const error = useResultStore((s) => s.error);
  const cancel = useResultStore((s) => s.cancel);
  const streamId = useResultStore((s) => s.streamId);
  const lastDsl = useResultStore((s) => s.lastDsl);
  const activeView = useViewStore((s) => s.activeView);
  const hasPostFilter = lastDsl?.post_filter != null;
  // Log 뷰는 자체 [복사] 버튼이 헤더에 있으므로 ResultBar의 결과 액션을 숨긴다
  // (보이는 데이터와 복사 대상이 어긋나는 혼동 방지).
  const showResultActions = activeView !== "log";
  const [busy, setBusy] = useState<"export" | "count" | "copy" | null>(null);

  const doExport = async (format: ExportFormat, source: ExportSource) => {
    if (!streamId || busy) return;
    const defaultExt = format === "ndjson" ? "ndjson" : format;
    const target = await save({
      defaultPath: `firescope-${Date.now()}.${defaultExt}`,
      filters: [{ name: format.toUpperCase(), extensions: [defaultExt] }],
    });
    if (!target) return;
    setBusy("export");
    try {
      const res = await exportResult({
        stream_id: streamId,
        format,
        path: target,
        source,
      });
      toast.success(
        `${res.row_count.toLocaleString()}건 저장 (${formatBytes(res.written_bytes)})`,
      );
    } catch (err) {
      toast.error(asAppError(err).message);
    } finally {
      setBusy(null);
    }
  };

  const doCopy = async () => {
    if (!rowsData.length || busy) return;
    setBusy("copy");
    try {
      const payload = JSON.stringify(rowsData, null, 2);
      await writeText(payload);
      toast.success(`${rowsData.length.toLocaleString()}건 클립보드 복사`);
    } catch (err) {
      toast.error(asAppError(err).message);
    } finally {
      setBusy(null);
    }
  };

  const doCount = async () => {
    if (!lastDsl || busy) return;
    setBusy("count");
    try {
      const res = await queryCount(lastDsl);
      toast.info(
        res.scanned > res.matched
          ? `전체 ${res.scanned.toLocaleString()}건 중 ${res.matched.toLocaleString()}건 매칭`
          : `${res.matched.toLocaleString()}건`,
      );
    } catch (err) {
      toast.error(asAppError(err).message);
    } finally {
      setBusy(null);
    }
  };

  const canExport = status === "done" && streamId != null && rows > 0;
  const canCopy = status === "done" && rows > 0;
  const canCount = status === "done" && lastDsl != null;

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
          <>
            <span className="text-muted-foreground">
              {scanned > total
                ? `전체 ${scanned}건 중 ${total}건 매칭`
                : `${total}건`}
              {tookMs != null ? ` · ${tookMs}ms` : ""}
            </span>
            {showResultActions && (
              <>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-7 gap-1 px-2 text-xs"
                  disabled={!canCopy || busy != null}
                  onClick={() => void doCopy()}
                  title="결과를 JSON으로 클립보드에 복사"
                >
                  <ClipboardCopy className="size-3.5" />
                  복사
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-7 gap-1 px-2 text-xs"
                  disabled={!canCount || busy != null}
                  onClick={() => void doCount()}
                  title="DSL을 다시 실행하여 카운트 계산"
                >
                  <Calculator className="size-3.5" />
                  카운트
                </Button>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-7 gap-1 px-2 text-xs"
                  disabled={!canExport || busy != null}
                  title="결과를 파일로 내보내기"
                >
                  <Download className="size-3.5" />
                  내보내기
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="text-xs">
                <DropdownMenuLabel className="text-xs">
                  매칭 결과 ({total.toLocaleString()}건)
                </DropdownMenuLabel>
                <DropdownMenuItem onClick={() => void doExport("json", "matched")}>
                  JSON
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => void doExport("ndjson", "matched")}>
                  NDJSON
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => void doExport("csv", "matched")}>
                  CSV
                </DropdownMenuItem>
                {hasPostFilter && (
                  <>
                    <DropdownMenuSeparator />
                    <DropdownMenuLabel className="text-xs">
                      후처리 이전 전체 ({scanned.toLocaleString()}건)
                    </DropdownMenuLabel>
                    <DropdownMenuItem onClick={() => void doExport("json", "scanned")}>
                      JSON
                    </DropdownMenuItem>
                    <DropdownMenuItem onClick={() => void doExport("ndjson", "scanned")}>
                      NDJSON
                    </DropdownMenuItem>
                    <DropdownMenuItem onClick={() => void doExport("csv", "scanned")}>
                      CSV
                    </DropdownMenuItem>
                  </>
                )}
              </DropdownMenuContent>
            </DropdownMenu>
              </>
            )}
          </>
        )}
        {status === "error" && (
          <span className="text-destructive">{error}</span>
        )}
      </span>
    </div>
  );
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}
