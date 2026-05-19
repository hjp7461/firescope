import { Tree, type NodeRendererProps } from "react-arborist";
import { useMemo } from "react";
import { ChevronRight, ChevronDown } from "lucide-react";
import { useResultStore } from "@/stores/resultStore";
import { buildTree, type TreeNode } from "@/lib/tree";
import { useElementSize } from "@/lib/useElementSize";
import { cn } from "@/lib/utils";

function Row({ node, style }: NodeRendererProps<TreeNode>) {
  const d = node.data;
  const hasChildren = !!d.children?.length;
  return (
    <div
      style={style}
      className="flex items-center border-b text-xs hover:bg-accent/40"
      onClick={() => node.toggle()}
    >
      <div className="flex min-w-0 flex-1 items-center gap-1 px-2 py-1.5">
        <span className="w-4 shrink-0 text-muted-foreground">
          {hasChildren ? (
            node.isOpen ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />
          ) : null}
        </span>
        <span className="truncate font-medium">{d.k}</span>
      </div>
      <div className="w-[45%] truncate px-2 py-1.5 text-muted-foreground" title={d.valuePreview}>
        {d.valuePreview}
      </div>
      <div className={cn("w-24 shrink-0 px-2 py-1.5 text-right", "text-primary/70")}>
        {d.typeLabel}
      </div>
    </div>
  );
}

export function TreeView() {
  const rows = useResultStore((s) => s.rows);
  const status = useResultStore((s) => s.status);
  const data = useMemo(() => buildTree(rows), [rows]);
  const [ref, { width, height }] = useElementSize<HTMLDivElement>();

  if (rows.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        {status === "streaming" ? "불러오는 중…" : "결과 없음"}
      </div>
    );
  }
  return (
    <div ref={ref} className="h-full overflow-hidden">
      <div className="sticky top-0 z-10 flex border-b bg-background text-xs font-semibold">
        <div className="flex-1 px-2 py-1.5">Key</div>
        <div className="w-[45%] px-2 py-1.5">Value</div>
        <div className="w-24 px-2 py-1.5 text-right">Type</div>
      </div>
      {width > 0 && height > 0 && (
        <Tree<TreeNode>
          data={data}
          idAccessor="id"
          openByDefault={false}
          width={width}
          height={height - 30}
          rowHeight={30}
          indent={16}
        >
          {Row}
        </Tree>
      )}
    </div>
  );
}
