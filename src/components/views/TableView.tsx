import { useMemo, useRef } from "react";
import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { cn } from "@/lib/utils";
import { useResultStore } from "@/stores/resultStore";
import { renderValue, type FirestoreDocument } from "@/types";

const ROW_HEIGHT = 33;
const col = createColumnHelper<FirestoreDocument>();

export function TableView() {
  const rows = useResultStore((s) => s.rows);
  const status = useResultStore((s) => s.status);

  // 컬럼 자동 감지: 로드된 문서들의 data 키 합집합 + 선행 id.
  const columns = useMemo(() => {
    const keys = new Set<string>();
    for (const d of rows) for (const k of Object.keys(d.data)) keys.add(k);
    return [
      col.accessor("id", {
        header: "id",
        size: 220,
        cell: (c) => {
          const id = c.getValue();
          return (
            <span className="font-mono text-xs" title={id}>
              {id}
            </span>
          );
        },
      }),
      ...[...keys].map((k) =>
        col.display({
          id: k,
          header: k,
          cell: ({ row }) => {
            const v = row.original.data[k];
            if (v === undefined)
              return <span className="text-muted-foreground">—</span>;
            const text = renderValue(v);
            return (
              <span className="text-xs" title={text}>
                {text}
              </span>
            );
          },
        }),
      ),
    ];
  }, [rows]);

  const table = useReactTable({
    data: rows,
    columns,
    defaultColumn: { size: 180, minSize: 64, maxSize: 800 },
    columnResizeMode: "onChange",
    getCoreRowModel: getCoreRowModel(),
  });

  const parentRef = useRef<HTMLDivElement>(null);
  const tableRows = table.getRowModel().rows;
  const virtualizer = useVirtualizer({
    count: tableRows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });

  if (rows.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        {status === "streaming" ? "불러오는 중…" : "결과 없음"}
      </div>
    );
  }

  const totalWidth = table.getTotalSize();

  return (
    <div ref={parentRef} className="h-full overflow-auto">
      <div style={{ width: totalWidth, position: "relative" }}>
        {/* 헤더 (sticky) — 컬럼 경계 드래그로 폭 조정 */}
        <div className="sticky top-0 z-10 flex border-b bg-background">
          {table.getFlatHeaders().map((h) => (
            <div
              key={h.id}
              className="relative shrink-0 px-3 py-1.5 text-xs font-semibold"
              style={{ width: h.getSize() }}
            >
              <span className="block truncate">
                {flexRender(h.column.columnDef.header, h.getContext())}
              </span>
              <div
                onMouseDown={h.getResizeHandler()}
                onTouchStart={h.getResizeHandler()}
                className={cn(
                  "absolute right-0 top-0 h-full w-1 cursor-col-resize select-none touch-none hover:bg-primary/40",
                  h.column.getIsResizing() && "bg-primary",
                )}
              />
            </div>
          ))}
        </div>

        {/* 가상화 바디 */}
        <div
          style={{
            height: virtualizer.getTotalSize(),
            position: "relative",
          }}
        >
          {virtualizer.getVirtualItems().map((vr) => {
            const r = tableRows[vr.index];
            return (
              <div
                key={r.id}
                className="absolute flex border-b hover:bg-accent/40"
                style={{
                  top: 0,
                  height: ROW_HEIGHT,
                  width: totalWidth,
                  transform: `translateY(${vr.start}px)`,
                }}
              >
                {r.getVisibleCells().map((cell) => (
                  <div
                    key={cell.id}
                    className="shrink-0 truncate px-3 py-1.5"
                    style={{ width: cell.column.getSize() }}
                  >
                    {flexRender(
                      cell.column.columnDef.cell,
                      cell.getContext(),
                    )}
                  </div>
                ))}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
