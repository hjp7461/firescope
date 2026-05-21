import { useEffect, useMemo, useRef } from "react";
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
/** 마지막 가상 행 인덱스가 `rows.length - 이 값` 이상이면 다음 페이지를 미리 받는다. */
const PREFETCH_THRESHOLD = 15;
const col = createColumnHelper<FirestoreDocument>();

export function TableView() {
  const rows = useResultStore((s) => s.rows);
  const status = useResultStore((s) => s.status);
  const hasMore = useResultStore((s) => s.hasMore);
  const fetchMoreInFlight = useResultStore((s) => s.fetchMoreInFlight);
  const listenerId = useResultStore((s) => s.listenerId);
  const fetchMore = useResultStore((s) => s.fetchMore);
  // 새 쿼리/문서 선택 시 lastDsl 참조가 바뀐다 (fetchMore는 lastDsl을
  // 그대로 두므로 참조 동일). 이 신호로 스크롤 위치를 0으로 리셋해야 새
  // 결과에서 이전 스크롤 위치 때문에 의도치 않은 fetchMore가 발동되는
  // 것을 막을 수 있다.
  const lastDsl = useResultStore((s) => s.lastDsl);

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

  // 사용자가 실제로 스크롤했는지 추적하는 플래그.
  //
  // `scrollOffset > 0`만으로는 부족하다: 새 컬렉션 클릭 시 청크가 빨리 도착해
  // 빈 placeholder 단계 없이 parentRef DOM이 재사용되면 이전 scrollTop이
  // 보존되고, virtualizer가 그걸 maxScroll로 clamp해도 여전히 양수라 자동
  // fetchMore가 발동한다. 명시적인 사용자 스크롤 이벤트만 신뢰한다.
  const userScrolledRef = useRef(false);
  // 새 쿼리(lastDsl 참조 변경)가 시작되면 스크롤 위치/플래그 리셋.
  // fetchMore는 lastDsl을 갱신하지 않으므로 이 effect는 안 흔들림.
  useEffect(() => {
    userScrolledRef.current = false;
    if (parentRef.current) parentRef.current.scrollTop = 0;
  }, [lastDsl]);

  // 가시 마지막 행이 끝 근처에 도달하면 다음 페이지를 자동으로 요청한다.
  // store가 hasMore/fetchMoreInFlight/listenerId 가드를 가지므로 중복 호출은
  // 안전하게 차단된다 (디바운스 불필요).
  const virtualItems = virtualizer.getVirtualItems();
  const lastVisibleIndex =
    virtualItems.length > 0 ? virtualItems[virtualItems.length - 1].index : -1;
  useEffect(() => {
    if (listenerId != null) return;
    if (!hasMore || fetchMoreInFlight) return;
    if (rows.length === 0) return;
    if (!userScrolledRef.current) return;
    if (lastVisibleIndex < rows.length - PREFETCH_THRESHOLD) return;
    void fetchMore();
  }, [lastVisibleIndex, rows.length, hasMore, fetchMoreInFlight, listenerId, fetchMore]);

  if (rows.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        {status === "streaming" ? "불러오는 중…" : "결과 없음"}
      </div>
    );
  }

  const totalWidth = table.getTotalSize();

  return (
    <div
      ref={parentRef}
      className="h-full overflow-auto"
      onScroll={() => {
        userScrolledRef.current = true;
      }}
    >
      {/* minWidth:100% → 컬럼 합이 좁아도 그리드가 패널 폭을 채움.
          넓으면 width=totalWidth가 가로 스크롤을 만든다. */}
      <div style={{ width: totalWidth, minWidth: "100%", position: "relative" }}>
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
          {/* 남는 폭 흡수용 필러 (상호작용 없음) */}
          <div className="flex-1" aria-hidden />
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
                className="absolute inset-x-0 flex border-b hover:bg-accent/40"
                style={{
                  top: 0,
                  height: ROW_HEIGHT,
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
                <div className="flex-1" aria-hidden />
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
