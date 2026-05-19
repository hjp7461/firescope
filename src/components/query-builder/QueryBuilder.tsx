import { useState } from "react";
import {
  Play,
  Plus,
  RotateCcw,
  Code2,
  Clock,
  Filter,
  CaseSensitive,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { useQueryStore } from "@/stores/queryStore";
import { useResultStore } from "@/stores/resultStore";
import { WhereRow } from "./WhereRow";
import { OrderByRow } from "./OrderByRow";
import { DslPreview } from "./DslPreview";
import { HistoryPanel } from "./HistoryPanel";

type SidePanel = "none" | "dsl" | "history";

function SectionHeader({
  title,
  onAdd,
}: {
  title: string;
  onAdd: () => void;
}) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
        {title}
      </span>
      <Button
        type="button"
        size="sm"
        variant="ghost"
        className="h-6 gap-1 px-1.5 text-xs"
        onClick={onAdd}
      >
        <Plus className="size-3" />
        추가
      </Button>
    </div>
  );
}

function PostFilterSection() {
  const pf = useQueryStore((s) => s.postFilter);
  const update = useQueryStore((s) => s.updatePostFilter);

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-1.5">
        <Filter className="size-3 text-muted-foreground" />
        <span className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
          후처리 검색
        </span>
        <span className="text-[10px] text-muted-foreground">
          (가져온 결과를 클라이언트에서 필터 — limit 권장)
        </span>
      </div>

      <div className="flex items-center gap-1.5">
        <Select
          value={pf.kind}
          onValueChange={(v) => update({ kind: v as "regex" | "contains" })}
        >
          <SelectTrigger
            className="h-7 w-[110px] text-xs"
            aria-label="후처리 종류"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="regex" className="text-xs">
              정규식
            </SelectItem>
            <SelectItem value="contains" className="text-xs">
              포함
            </SelectItem>
          </SelectContent>
        </Select>
        <Input
          value={pf.fields}
          onChange={(e) => update({ fields: e.target.value })}
          placeholder="필드 (쉼표 구분, 예: name, profile.city)"
          className="h-7 w-[40%] text-xs"
          aria-label="후처리 필드"
        />
        <Input
          value={pf.pattern}
          onChange={(e) => update({ pattern: e.target.value })}
          placeholder={pf.kind === "regex" ? "정규식 패턴" : "포함 문자열"}
          className="h-7 flex-1 text-xs"
          aria-label="후처리 패턴"
        />
        <Button
          type="button"
          size="sm"
          variant={pf.caseInsensitive ? "secondary" : "ghost"}
          className="h-7 gap-1 px-2 text-xs"
          aria-pressed={pf.caseInsensitive}
          title="대소문자 무시"
          onClick={() => update({ caseInsensitive: !pf.caseInsensitive })}
        >
          <CaseSensitive className="size-3.5" />
          Aa
        </Button>
      </div>

      <Input
        value={pf.jsonpath}
        onChange={(e) => update({ jsonpath: e.target.value })}
        placeholder="JSONPath (선택, 예: $.tags[?@ == 'urgent'])"
        className="h-7 text-xs"
        aria-label="후처리 JSONPath"
      />
    </div>
  );
}

export function QueryBuilder() {
  const s = useQueryStore();
  const build = useQueryStore((st) => st.build);
  const runDsl = useResultStore((st) => st.runDsl);
  const [error, setError] = useState<string | null>(null);
  const [side, setSide] = useState<SidePanel>("none");

  const toggleSide = (panel: Exclude<SidePanel, "none">) =>
    setSide((cur) => (cur === panel ? "none" : panel));

  async function onRun() {
    const r = build();
    if (!r.ok) {
      setError(r.error);
      return;
    }
    setError(null);
    await runDsl(r.dsl);
  }

  return (
    <div className="flex max-h-[42vh] flex-col gap-3 border-b bg-muted/20 p-3">
      <div className="flex items-center gap-1.5">
        <Select
          value={s.targetKind}
          onValueChange={(v) =>
            s.setTargetKind(v as "collection" | "collection_group")
          }
        >
          <SelectTrigger className="h-7 w-[150px] text-xs" aria-label="대상 종류">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="collection" className="text-xs">
              collection
            </SelectItem>
            <SelectItem value="collection_group" className="text-xs">
              collection group
            </SelectItem>
          </SelectContent>
        </Select>
        <Input
          value={s.target}
          onChange={(e) => s.setTarget(e.target.value)}
          placeholder={
            s.targetKind === "collection"
              ? "users 또는 users/abc/posts"
              : "그룹 ID (예: comments)"
          }
          className="h-7 flex-1 text-xs"
          aria-label="대상"
        />
        <span className="text-xs text-muted-foreground">limit</span>
        <Input
          type="number"
          value={s.limit}
          min={0}
          max={1000}
          onChange={(e) => s.setLimit(Number(e.target.value))}
          className="h-7 w-[72px] text-xs"
          aria-label="limit"
        />
        <Button
          type="button"
          size="sm"
          variant={side === "history" ? "secondary" : "ghost"}
          className="h-7 gap-1 px-2 text-xs"
          aria-pressed={side === "history"}
          onClick={() => toggleSide("history")}
        >
          <Clock className="size-3.5" />
          히스토리
        </Button>
        <Button
          type="button"
          size="sm"
          variant={side === "dsl" ? "secondary" : "ghost"}
          className="h-7 gap-1 px-2 text-xs"
          aria-pressed={side === "dsl"}
          onClick={() => toggleSide("dsl")}
        >
          <Code2 className="size-3.5" />
          DSL
        </Button>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          className="h-7 gap-1 px-2 text-xs"
          onClick={() => {
            s.reset();
            setError(null);
          }}
        >
          <RotateCcw className="size-3.5" />
          초기화
        </Button>
        <Button
          type="button"
          size="sm"
          className="h-7 gap-1 px-3 text-xs"
          onClick={() => void onRun()}
        >
          <Play className="size-3.5" />
          실행
        </Button>
      </div>

      <div className={cn("flex min-h-0 flex-1 gap-3")}>
        <ScrollArea className="min-h-0 flex-1">
          <div className="flex flex-col gap-3 pr-3">
            <div className="flex flex-col gap-1.5">
              <SectionHeader title="Where (AND)" onAdd={s.addWhere} />
              {s.wheres.length === 0 ? (
                <p className="py-1 text-xs text-muted-foreground">
                  조건 없음 — 전체 조회
                </p>
              ) : (
                s.wheres.map((w, i) => (
                  <WhereRow
                    key={i}
                    where={w}
                    onChange={(patch) => s.updateWhere(i, patch)}
                    onRemove={() => s.removeWhere(i)}
                  />
                ))
              )}
            </div>

            <Separator />

            <div className="flex flex-col gap-1.5">
              <SectionHeader title="Order By" onAdd={s.addOrderBy} />
              {s.orderBys.length === 0 ? (
                <p className="py-1 text-xs text-muted-foreground">
                  정렬 없음
                </p>
              ) : (
                s.orderBys.map((o, i) => (
                  <OrderByRow
                    key={i}
                    order={o}
                    onChange={(patch) => s.updateOrderBy(i, patch)}
                    onRemove={() => s.removeOrderBy(i)}
                  />
                ))
              )}
            </div>

            <Separator />

            <PostFilterSection />

            {error && (
              <p className="rounded-md bg-destructive/10 px-2 py-1.5 text-xs text-destructive">
                {error}
              </p>
            )}
          </div>
        </ScrollArea>

        {side !== "none" && (
          <div className="min-h-0 w-[40%] overflow-hidden rounded-md border bg-background">
            {side === "dsl" ? <DslPreview /> : <HistoryPanel />}
          </div>
        )}
      </div>
    </div>
  );
}
