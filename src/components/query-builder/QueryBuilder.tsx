import { useState } from "react";
import { Play, Plus, RotateCcw, Code2 } from "lucide-react";
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

export function QueryBuilder() {
  const s = useQueryStore();
  const build = useQueryStore((st) => st.build);
  const runDsl = useResultStore((st) => st.runDsl);
  const [error, setError] = useState<string | null>(null);
  const [showPreview, setShowPreview] = useState(false);

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
          variant={showPreview ? "secondary" : "ghost"}
          className="h-7 gap-1 px-2 text-xs"
          aria-pressed={showPreview}
          onClick={() => setShowPreview((v) => !v)}
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

            {error && (
              <p className="rounded-md bg-destructive/10 px-2 py-1.5 text-xs text-destructive">
                {error}
              </p>
            )}
          </div>
        </ScrollArea>

        {showPreview && (
          <div className="min-h-0 w-[40%] overflow-hidden rounded-md border bg-background">
            <DslPreview />
          </div>
        )}
      </div>
    </div>
  );
}
