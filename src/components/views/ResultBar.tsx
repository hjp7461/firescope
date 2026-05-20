import { useState } from "react";
import {
  BarChart3,
  Calculator,
  ClipboardCopy,
  Download,
  Radio,
  SlidersHorizontal,
} from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
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
import { type ExportFormat, type ExportSource, type ProfileMode } from "@/types";
import { toKoreanMessage } from "@/lib/errorMessages";
import { ViewTabs } from "./ViewTabs";
import { StatsDialog } from "./StatsDialog";

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
  const indexUrl = useResultStore((s) => s.indexUrl);
  const cancel = useResultStore((s) => s.cancel);
  const streamId = useResultStore((s) => s.streamId);
  const lastDsl = useResultStore((s) => s.lastDsl);
  const listenerId = useResultStore((s) => s.listenerId);
  const listenerStatus = useResultStore((s) => s.listenerStatus);
  const listenerEventCount = useResultStore((s) => s.listenerEventCount);
  const startListening = useResultStore((s) => s.startListening);
  const stopListening = useResultStore((s) => s.stopListening);
  const activeView = useViewStore((s) => s.activeView);
  const hasPostFilter = lastDsl?.post_filter != null;
  // Log 뷰는 자체 [복사] 버튼이 헤더에 있으므로 ResultBar의 결과 액션을 숨긴다
  // (보이는 데이터와 복사 대상이 어긋나는 혼동 방지).
  const showResultActions = activeView !== "log";
  const [busy, setBusy] = useState<"export" | "count" | "copy" | null>(null);
  const [statsOpen, setStatsOpen] = useState(false);

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
      toast.error(toKoreanMessage(err));
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
      toast.error(toKoreanMessage(err));
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
      toast.error(toKoreanMessage(err));
    } finally {
      setBusy(null);
    }
  };

  const canExport = status === "done" && streamId != null && rows > 0;
  const canCopy = status === "done" && rows > 0;
  const canCount = status === "done" && lastDsl != null;
  const canStats = status === "done" && streamId != null && rows > 0;
  const isListening = listenerId != null;
  const canToggleListener = isListening || (lastDsl != null && lastDsl.target != null);

  const toggleListener = async () => {
    if (isListening) {
      try {
        await stopListening();
      } catch (err) {
        toast.error(toKoreanMessage(err));
      }
      return;
    }
    if (!lastDsl) return;
    try {
      // listener는 DSL 서브셋만 사용 — order_by/limit/select/cursor/post_filter는 무시.
      await startListening({
        target: lastDsl.target,
        where: lastDsl.where,
      });
    } catch (err) {
      toast.error(toKoreanMessage(err));
    }
  };

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
        {showResultActions && canToggleListener && (
          <Button
            type="button"
            size="sm"
            variant={isListening ? "secondary" : "ghost"}
            className="h-7 gap-1.5 px-2 text-xs"
            aria-pressed={isListening}
            onClick={() => void toggleListener()}
            title={
              isListening
                ? "Live 모드 종료 (스냅샷으로 복귀)"
                : "Realtime 리스너 시작 — 결과집합이 실시간 갱신됩니다"
            }
          >
            <Radio
              className={`size-3.5 ${isListening ? "animate-pulse text-red-500" : ""}`}
            />
            Live
          </Button>
        )}
        {status === "listening" && (
          <span className="flex items-center gap-1 text-muted-foreground">
            <span
              className={`inline-block size-1.5 rounded-full ${
                listenerStatus === "ready"
                  ? "bg-green-500"
                  : listenerStatus === "reset"
                    ? "bg-amber-500"
                    : "bg-muted-foreground animate-pulse"
              }`}
            />
            <span>
              Live · {rows}건
              {listenerEventCount > 0
                ? ` · ${listenerEventCount}회 변경`
                : ""}
            </span>
          </span>
        )}
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
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-7 gap-1 px-2 text-xs"
                  disabled={!canStats}
                  onClick={() => setStatsOpen(true)}
                  title="필드별 타입 분포·NULL 비율·상위 값"
                >
                  <BarChart3 className="size-3.5" />
                  통계
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
          <span className="flex items-center gap-2 text-destructive">
            {error}
            {indexUrl && (
              <button
                type="button"
                onClick={() => void openUrl(indexUrl)}
                className="rounded border border-destructive/40 px-1.5 py-0.5 text-[10px] font-medium hover:bg-destructive/10"
                title={`Firebase 콘솔에서 누락된 인덱스 생성: ${indexUrl}`}
              >
                인덱스 생성 ↗
              </button>
            )}
          </span>
        )}
      </span>
      <StatsDialog open={statsOpen} onOpenChange={setStatsOpen} />
    </div>
  );
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}
