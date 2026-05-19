import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Folder, RefreshCw } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { listCollections } from "@/ipc/query";
import { useResultStore } from "@/stores/resultStore";
import { useSessionStore } from "@/stores/sessionStore";
import { asAppError } from "@/types";

// 활성 세션의 루트 컬렉션 목록. 클릭 → 해당 컬렉션 첫 100건 쿼리.
export function CollectionsPanel() {
  const sessionId = useSessionStore((s) => s.current?.session_id ?? null);
  const [collections, setCollections] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const activePath = useResultStore((s) => s.collectionPath);
  const runQuery = useResultStore((s) => s.runCollectionQuery);

  async function load() {
    setLoading(true);
    try {
      setCollections(await listCollections());
    } catch (err) {
      toast.error(asAppError(err).message);
    } finally {
      setLoading(false);
    }
  }

  // 세션이 바뀌면 컬렉션 목록 재조회.
  useEffect(() => {
    if (sessionId) void load();
    else setCollections([]);
  }, [sessionId]);

  return (
    <div className="flex h-full w-56 flex-col border-r">
      <div className="flex items-center justify-between px-3 py-2">
        <span className="text-xs font-semibold">컬렉션</span>
        <Button
          size="sm"
          variant="ghost"
          onClick={load}
          disabled={loading}
          aria-label="새로고침"
        >
          <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
        </Button>
      </div>
      <ScrollArea className="flex-1 px-2 pb-2">
        {collections.length === 0 ? (
          <p className="px-2 py-4 text-xs text-muted-foreground">
            {loading ? "불러오는 중…" : "컬렉션 없음"}
          </p>
        ) : (
          <ul className="space-y-0.5">
            {collections.map((c) => (
              <li key={c}>
                <button
                  type="button"
                  onClick={() => runQuery(c)}
                  className={cn(
                    "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors",
                    activePath === c
                      ? "bg-accent text-accent-foreground"
                      : "hover:bg-accent/50",
                  )}
                >
                  <Folder className="size-4 shrink-0 text-muted-foreground" />
                  <span className="truncate">{c}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </ScrollArea>
    </div>
  );
}
