import { useMemo, useRef } from "react";
import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
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
        cell: (c) => <span className="font-mono text-xs">{c.getValue()}</span>,
      }),
      ...[...keys].map((k) =>
        col.display({
          id: k,
          header: k,
          cell: ({ row }) => {
            const v = row.original.data[k];
            return (
              <span className="text-xs" title={v ? renderValue(v) : ""}>
                {v === undefined ? (
                  <span className="text-muted-foreground">—</span>
                ) : (
                  renderValue(v)
                )}
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

  const virtualRows = virtualizer.getVirtualItems();

  return (
    <div ref={parentRef} className="h-full overflow-auto">
      <table className="w-full border-collapse text-left">
        <thead className="sticky top-0 z-10 bg-background">
          {table.getHeaderGroups().map((hg) => (
            <tr key={hg.id} className="border-b">
              {hg.headers.map((h) => (
                <th
                  key={h.id}
                  className="whitespace-nowrap px-3 py-1.5 text-xs font-semibold"
                >
                  {flexRender(h.column.columnDef.header, h.getContext())}
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody
          style={{
            height: `${virtualizer.getTotalSize()}px`,
            position: "relative",
            display: "block",
          }}
        >
          {virtualRows.map((vr) => {
            const r = tableRows[vr.index];
            return (
              <tr
                key={r.id}
                className="absolute flex w-full border-b hover:bg-accent/40"
                style={{
                  height: `${ROW_HEIGHT}px`,
                  transform: `translateY(${vr.start}px)`,
                }}
              >
                {r.getVisibleCells().map((cell) => (
                  <td
                    key={cell.id}
                    className="flex-1 truncate px-3 py-1.5"
                  >
                    {flexRender(
                      cell.column.columnDef.cell,
                      cell.getContext(),
                    )}
                  </td>
                ))}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
