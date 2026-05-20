import { useEffect, useMemo, useRef, useState } from "react";
import { BarChart3, Loader2 } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useResultStore } from "@/stores/resultStore";
import { computeStats } from "@/ipc/query";
import { percent, typeColor } from "@/lib/stats";
import { toKoreanMessage } from "@/lib/errorMessages";
import type {
  ExportSource,
  FieldStat,
  StatsReport,
} from "@/types";
import { cn } from "@/lib/utils";

const SAMPLE_SIZES = [100, 500, 1000] as const;
type SampleSize = (typeof SAMPLE_SIZES)[number];
const DEFAULT_SAMPLE_SIZE: SampleSize = 500;
const TOP_SAMPLES = 5;
const NESTED_DEPTHS = [1, 2, 3, 5] as const;
type NestedDepth = (typeof NESTED_DEPTHS)[number];
const DEFAULT_NESTED_DEPTH: NestedDepth = 3;

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

/**
 * 컬렉션 통계 모달 (`docs/06-roadmap.md` Phase 9-C).
 *
 * `resultStore`를 직접 구독하여 현재 활성 sink로부터 통계를 계산한다.
 * 샘플 크기가 현재 결과보다 크면 동일 DSL을 새 limit으로 자동 재실행하고,
 * 새 sink가 완료되면 자동으로 통계 IPC를 재호출한다.
 */
export function StatsDialog({ open, onOpenChange }: Props) {
  const status = useResultStore((s) => s.status);
  const streamId = useResultStore((s) => s.streamId);
  const lastDsl = useResultStore((s) => s.lastDsl);
  const runDsl = useResultStore((s) => s.runDsl);

  const hasPostFilter = lastDsl?.post_filter != null;

  const [sampleSize, setSampleSize] = useState<SampleSize>(DEFAULT_SAMPLE_SIZE);
  const [source, setSource] = useState<ExportSource>("matched");
  const [includeNested, setIncludeNested] = useState(false);
  const [nestedDepth, setNestedDepth] = useState<NestedDepth>(DEFAULT_NESTED_DEPTH);
  const [report, setReport] = useState<StatsReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // 동일 sink+source+nested 조합 중복 호출 방지.
  const lastComputed = useRef<{
    streamId: string;
    source: ExportSource;
    includeNested: boolean;
    nestedDepth: NestedDepth;
  } | null>(null);

  // 모달이 닫힐 때 상태를 초기화한다 (다음 오픈 시 신선한 시작).
  useEffect(() => {
    if (!open) {
      setReport(null);
      setError(null);
      lastComputed.current = null;
    }
  }, [open]);

  // 1) 샘플 크기가 직전 쿼리의 limit보다 크면 자동 재실행.
  //    `lastDsl.limit` 비교로 무한루프 방지 — 컬렉션이 작아 N건을 못 채워도
  //    limit이 sampleSize와 같으면 더 가져올 게 없다는 뜻이므로 재시도 안 함.
  useEffect(() => {
    if (!open || !lastDsl) return;
    if (status !== "done") return;
    const currentLimit = lastDsl.limit ?? Number.POSITIVE_INFINITY;
    if (currentLimit >= sampleSize) return;
    void runDsl({ ...lastDsl, limit: sampleSize });
  }, [open, sampleSize, status, lastDsl, runDsl]);

  // 2) 활성 sink가 done이면 통계 IPC 호출.
  useEffect(() => {
    if (!open) return;
    if (status !== "done" || !streamId) return;
    const fp = lastComputed.current;
    if (
      fp &&
      fp.streamId === streamId &&
      fp.source === source &&
      fp.includeNested === includeNested &&
      fp.nestedDepth === nestedDepth
    ) {
      return;
    }
    setBusy(true);
    setError(null);
    void computeStats({
      stream_id: streamId,
      source,
      top_samples: TOP_SAMPLES,
      include_nested: includeNested,
      max_depth: nestedDepth,
    })
      .then((r) => {
        lastComputed.current = { streamId, source, includeNested, nestedDepth };
        setReport(r);
      })
      .catch((err) => setError(toKoreanMessage(err)))
      .finally(() => setBusy(false));
  }, [open, status, streamId, source, includeNested, nestedDepth]);

  const isStreaming = status === "streaming";
  const fields = report?.fields ?? [];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <BarChart3 className="size-5" />
            컬렉션 통계
          </DialogTitle>
          <DialogDescription>
            샘플 문서의 필드별 타입 분포·NULL/누락 비율·상위 값.
            nested 필드 내부는 노출되지 않습니다.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-wrap items-center gap-2 border-y py-2 text-xs">
          <span className="text-muted-foreground">샘플</span>
          <div className="flex rounded-md border bg-muted/40 p-0.5">
            {SAMPLE_SIZES.map((n) => (
              <button
                key={n}
                type="button"
                onClick={() => setSampleSize(n)}
                className={cn(
                  "rounded px-2 py-0.5 text-xs transition-colors",
                  sampleSize === n
                    ? "bg-background font-medium shadow-sm"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                {n.toLocaleString()}
              </button>
            ))}
          </div>

          {hasPostFilter && (
            <>
              <span className="ml-2 text-muted-foreground">대상</span>
              <div className="flex rounded-md border bg-muted/40 p-0.5">
                {(["matched", "scanned"] as const).map((s) => (
                  <button
                    key={s}
                    type="button"
                    onClick={() => setSource(s)}
                    className={cn(
                      "rounded px-2 py-0.5 text-xs transition-colors",
                      source === s
                        ? "bg-background font-medium shadow-sm"
                        : "text-muted-foreground hover:text-foreground",
                    )}
                  >
                    {s === "matched" ? "매칭" : "전체"}
                  </button>
                ))}
              </div>
            </>
          )}

          <label className="ml-2 flex items-center gap-1.5 text-xs">
            <input
              type="checkbox"
              checked={includeNested}
              onChange={(e) => setIncludeNested(e.target.checked)}
              className="size-3.5 rounded border-input"
            />
            <span className="text-muted-foreground">nested 펼치기</span>
          </label>

          {includeNested && (
            <div className="flex rounded-md border bg-muted/40 p-0.5">
              {NESTED_DEPTHS.map((d) => (
                <button
                  key={d}
                  type="button"
                  onClick={() => setNestedDepth(d)}
                  className={cn(
                    "rounded px-2 py-0.5 text-xs transition-colors",
                    nestedDepth === d
                      ? "bg-background font-medium shadow-sm"
                      : "text-muted-foreground hover:text-foreground",
                  )}
                  title={`최대 깊이 ${d}`}
                >
                  d{d}
                </button>
              ))}
            </div>
          )}

          <span className="ml-auto text-muted-foreground">
            {report
              ? `${report.sample_size.toLocaleString()}건 · ${report.fields.length}개 필드`
              : "—"}
          </span>
        </div>

        {error && (
          <div className="rounded border border-destructive/40 bg-destructive/5 p-2 text-xs text-destructive">
            {error}
          </div>
        )}

        <div className="min-h-[20rem]">
          {isStreaming || busy ? (
            <div className="flex h-72 items-center justify-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="size-4 animate-spin" />
              {isStreaming ? "샘플 수집 중…" : "통계 계산 중…"}
            </div>
          ) : fields.length === 0 ? (
            <div className="flex h-72 items-center justify-center text-sm text-muted-foreground">
              표시할 필드가 없습니다.
            </div>
          ) : (
            <ScrollArea className="h-[28rem] pr-3">
              <ul className="space-y-3">
                {fields.map((f) => (
                  <FieldRow key={f.key} stat={f} sampleSize={report!.sample_size} />
                ))}
              </ul>
            </ScrollArea>
          )}
        </div>

        <div className="flex justify-end gap-2 pt-2">
          <Button variant="ghost" size="sm" onClick={() => onOpenChange(false)}>
            닫기
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function FieldRow({
  stat,
  sampleSize,
}: {
  stat: FieldStat;
  sampleSize: number;
}) {
  // present 기준으로 type 분포의 100% 너비를 잡되, missing은 가장 우측에 회색.
  const total = sampleSize;
  const segments = useMemo(() => {
    const out: Array<{ color: string; ratio: number; label: string }> = [];
    for (const t of stat.types) {
      out.push({
        color: typeColor(t.type).bg,
        ratio: total > 0 ? t.count / total : 0,
        label: `${t.type}: ${t.count.toLocaleString()}건`,
      });
    }
    if (stat.missing > 0) {
      out.push({
        color: "bg-muted",
        ratio: total > 0 ? stat.missing / total : 0,
        label: `missing: ${stat.missing.toLocaleString()}건`,
      });
    }
    return out;
  }, [stat, total]);

  // dot-path의 가독성을 위해 depth만큼 leading indent + 부모.자식 dim.
  const segs = stat.key.split(".");
  const leaf = segs[segs.length - 1];
  const ancestors = segs.slice(0, -1).join(".");

  return (
    <li
      className={cn(
        "rounded border bg-card/30 p-2.5",
        stat.depth > 0 && "border-dashed bg-card/10",
      )}
      style={stat.depth > 0 ? { marginLeft: `${stat.depth * 0.75}rem` } : undefined}
    >
      <div className="mb-1.5 flex items-center justify-between gap-2">
        <span className="truncate font-mono text-sm">
          {ancestors && (
            <span className="text-muted-foreground/70">{ancestors}.</span>
          )}
          <span className="font-medium">{leaf}</span>
        </span>
        <div className="flex items-center gap-1.5 text-[10px]">
          {stat.null_count > 0 && (
            <Badge variant="secondary" className="px-1.5 py-0">
              null {percent(stat.null_count, total)}
            </Badge>
          )}
          {stat.missing > 0 && (
            <Badge variant="outline" className="px-1.5 py-0">
              missing {percent(stat.missing, total)}
            </Badge>
          )}
        </div>
      </div>

      <div
        className="flex h-2 w-full overflow-hidden rounded-sm bg-muted"
        role="img"
        aria-label={`${stat.key} 타입 분포`}
      >
        {segments.map((seg, i) => (
          <div
            key={i}
            className={cn(seg.color, "h-full")}
            style={{ width: `${seg.ratio * 100}%` }}
            title={seg.label}
          />
        ))}
      </div>

      <div className="mt-1.5 flex flex-wrap gap-1.5 text-[10px]">
        {stat.types.map((t) => {
          const c = typeColor(t.type);
          return (
            <span key={t.type} className="flex items-center gap-1">
              <span className={cn(c.bg, "size-2 rounded-sm")} />
              <span className={cn("font-mono", c.text)}>{t.type}</span>
              <span className="text-muted-foreground">
                {t.count.toLocaleString()} ({percent(t.count, total)})
              </span>
            </span>
          );
        })}
      </div>

      {stat.samples.length > 0 && (
        <div className="mt-2 space-y-0.5 text-xs">
          <div className="text-[10px] text-muted-foreground">상위 값</div>
          <ul className="space-y-0.5">
            {stat.samples.map((s, i) => (
              <li
                key={i}
                className="flex items-baseline justify-between gap-2 font-mono"
              >
                <span className="truncate text-foreground" title={s.value}>
                  {s.value}
                </span>
                <span className="shrink-0 text-muted-foreground">
                  {s.count.toLocaleString()} · {percent(s.count, total)}
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </li>
  );
}
