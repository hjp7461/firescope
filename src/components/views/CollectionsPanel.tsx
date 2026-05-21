import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { ChevronDown, ChevronRight, FileText, Folder, RefreshCw } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  listCollectionDocIds,
  listCollections,
  listSubcollections,
} from "@/ipc/query";
import { useResultStore } from "@/stores/resultStore";
import { useQueryStore } from "@/stores/queryStore";
import { useActiveSession } from "@/stores/tabsStore";
import { toKoreanMessage } from "@/lib/errorMessages";

// 컬렉션 → 문서 ID → 서브컬렉션 → ... 재귀 트리.
// - 컬렉션 노드 클릭: 빌더에 그 경로를 싣고 첫 100건 쿼리.
// - 문서 노드 클릭: 확장 토글 (서브컬렉션 lazy load).
// - 자식은 모두 lazy load — 펼치는 순간 IPC 호출 1회.
const DOC_PAGE_SIZE = 100;

type NodeKind = "collection" | "document";
type NodeChildrenState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "loaded"; ids: string[]; hasMore: boolean; nextPageToken?: string }
  | { status: "error"; message: string };

interface TreeNode {
  kind: NodeKind;
  /** 전체 Firestore 경로 (e.g. "Hospital" 또는 "Hospital/abc/ActivityLog"). */
  path: string;
  /** 마지막 세그먼트 — 표시용. */
  label: string;
  depth: number;
}

export function CollectionsPanel() {
  const sessionId = useActiveSession()?.session_id ?? null;
  const [rootCollections, setRootCollections] = useState<string[]>([]);
  const [loadingRoot, setLoadingRoot] = useState(false);
  // path → 펼침 여부 / 자식 로드 상태
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [children, setChildren] = useState<Record<string, NodeChildrenState>>({});
  const activePath = useResultStore((s) => s.collectionPath);
  const runQuery = useResultStore((s) => s.runCollectionQuery);
  const selectDocument = useResultStore((s) => s.selectDocument);
  const loadFromTarget = useQueryStore((s) => s.loadFromTarget);

  function onCollectionPick(path: string) {
    loadFromTarget("collection", path);
    void runQuery(path);
  }

  function onDocumentPick(path: string) {
    // 빌더는 "쿼리할 컬렉션"을 가리키는 입력이므로 문서가 아닌 그 부모
    // 컬렉션 경로로 갱신한다(예: `Hospital/abc` 선택 → 빌더 path는
    // `Hospital`). 빌더가 닫혀 있을 때 보이는 path 라인은 `resultStore`의
    // `collectionPath`(=문서 풀 경로)를 그대로 표시하므로 풀 경로도 확인 가능.
    const segs = path.split("/").filter(Boolean);
    if (segs.length >= 2) {
      const parentCollection = segs.slice(0, -1).join("/");
      loadFromTarget("collection", parentCollection);
    }
    void selectDocument(path);
  }

  const loadRoot = useCallback(async () => {
    setLoadingRoot(true);
    try {
      setRootCollections(await listCollections());
    } catch (err) {
      toast.error(toKoreanMessage(err));
    } finally {
      setLoadingRoot(false);
    }
  }, []);

  // 세션 전환 시 모든 캐시 초기화.
  useEffect(() => {
    setExpanded({});
    setChildren({});
    if (sessionId) void loadRoot();
    else setRootCollections([]);
  }, [sessionId, loadRoot]);

  /** 노드 펼치기 — 처음일 때만 자식 IPC 호출. */
  async function toggleNode(node: TreeNode) {
    const open = !expanded[node.path];
    setExpanded((m) => ({ ...m, [node.path]: open }));
    if (!open) return;

    const current = children[node.path];
    if (current && current.status !== "idle" && current.status !== "error") return;

    setChildren((m) => ({ ...m, [node.path]: { status: "loading" } }));
    try {
      if (node.kind === "collection") {
        const { collection_id, parent_path } = splitForList(node.path);
        const res = await listCollectionDocIds({
          collection_id,
          parent_path,
          page_size: DOC_PAGE_SIZE,
        });
        setChildren((m) => ({
          ...m,
          [node.path]: {
            status: "loaded",
            ids: res.doc_ids,
            hasMore: Boolean(res.page_token),
            nextPageToken: res.page_token,
          },
        }));
      } else {
        const ids = await listSubcollections(node.path);
        setChildren((m) => ({
          ...m,
          [node.path]: { status: "loaded", ids, hasMore: false },
        }));
      }
    } catch (err) {
      const message = toKoreanMessage(err);
      setChildren((m) => ({ ...m, [node.path]: { status: "error", message } }));
      toast.error(message);
    }
  }

  return (
    <div className="flex h-full w-64 flex-col border-r">
      <div className="flex items-center justify-between px-3 py-2">
        <span className="text-xs font-semibold">컬렉션</span>
        <Button
          size="sm"
          variant="ghost"
          onClick={loadRoot}
          disabled={loadingRoot}
          aria-label="새로고침"
        >
          <RefreshCw className={cn("size-3.5", loadingRoot && "animate-spin")} />
        </Button>
      </div>
      <ScrollArea className="flex-1 px-1 pb-2">
        {rootCollections.length === 0 ? (
          <p className="px-3 py-4 text-xs text-muted-foreground">
            {loadingRoot ? "불러오는 중…" : "컬렉션 없음"}
          </p>
        ) : (
          <ul className="space-y-0.5">
            {rootCollections.map((c) => (
              <CollectionBranch
                key={c}
                node={{ kind: "collection", path: c, label: c, depth: 0 }}
                activePath={activePath}
                expanded={expanded}
                children_={children}
                onToggle={toggleNode}
                onPick={onCollectionPick}
                onPickDocument={onDocumentPick}
              />
            ))}
          </ul>
        )}
      </ScrollArea>
    </div>
  );
}

interface BranchProps {
  node: TreeNode;
  activePath: string | null;
  expanded: Record<string, boolean>;
  children_: Record<string, NodeChildrenState>;
  onToggle: (node: TreeNode) => void;
  onPick: (path: string) => void;
  onPickDocument: (path: string) => void;
}

function CollectionBranch({
  node,
  activePath,
  expanded,
  children_,
  onToggle,
  onPick,
  onPickDocument,
}: BranchProps) {
  const isOpen = !!expanded[node.path];
  const state = children_[node.path];
  const isActive = activePath === node.path;

  return (
    <li>
      <div
        className={cn(
          "group flex items-center gap-1 rounded-md text-sm transition-colors",
          isActive ? "bg-accent text-accent-foreground" : "hover:bg-accent/50",
        )}
        style={{ paddingLeft: 4 + node.depth * 12 }}
      >
        <button
          type="button"
          onClick={() => onToggle(node)}
          aria-label={isOpen ? "접기" : "펼치기"}
          className="flex h-7 w-5 shrink-0 items-center justify-center text-muted-foreground hover:text-foreground"
        >
          {isOpen ? (
            <ChevronDown className="size-3.5" />
          ) : (
            <ChevronRight className="size-3.5" />
          )}
        </button>
        {node.kind === "collection" ? (
          <button
            type="button"
            onClick={() => onPick(node.path)}
            className="flex min-w-0 flex-1 items-center gap-2 py-1.5 text-left"
          >
            <Folder className="size-4 shrink-0 text-muted-foreground" />
            <span className="truncate">{node.label}</span>
          </button>
        ) : (
          <button
            type="button"
            onClick={() => onPickDocument(node.path)}
            className="flex min-w-0 flex-1 items-center gap-2 py-1.5 text-left"
          >
            <FileText className="size-4 shrink-0 text-muted-foreground" />
            <span className="truncate font-mono text-xs">{node.label}</span>
          </button>
        )}
      </div>
      {isOpen && (
        <ChildrenList
          parent={node}
          state={state}
          activePath={activePath}
          expanded={expanded}
          children_={children_}
          onToggle={onToggle}
          onPick={onPick}
          onPickDocument={onPickDocument}
        />
      )}
    </li>
  );
}

interface ChildrenListProps extends Omit<BranchProps, "node"> {
  parent: TreeNode;
  state: NodeChildrenState | undefined;
}

function ChildrenList({
  parent,
  state,
  activePath,
  expanded,
  children_,
  onToggle,
  onPick,
  onPickDocument,
}: ChildrenListProps) {
  if (!state || state.status === "idle" || state.status === "loading") {
    return (
      <p
        className="py-1 text-xs text-muted-foreground"
        style={{ paddingLeft: 4 + (parent.depth + 1) * 12 + 24 }}
      >
        불러오는 중…
      </p>
    );
  }
  if (state.status === "error") {
    return (
      <p
        className="py-1 text-xs text-destructive"
        style={{ paddingLeft: 4 + (parent.depth + 1) * 12 + 24 }}
      >
        {state.message}
      </p>
    );
  }
  if (state.ids.length === 0) {
    return (
      <p
        className="py-1 text-xs text-muted-foreground"
        style={{ paddingLeft: 4 + (parent.depth + 1) * 12 + 24 }}
      >
        비어 있음
      </p>
    );
  }
  return (
    <ul className="space-y-0.5">
      {state.ids.map((id) => {
        const childKind: NodeKind = parent.kind === "collection" ? "document" : "collection";
        const childPath = `${parent.path}/${id}`;
        return (
          <CollectionBranch
            key={childPath}
            node={{
              kind: childKind,
              path: childPath,
              label: id,
              depth: parent.depth + 1,
            }}
            activePath={activePath}
            expanded={expanded}
            children_={children_}
            onToggle={onToggle}
            onPick={onPick}
            onPickDocument={onPickDocument}
          />
        );
      })}
      {state.hasMore && (
        <li
          className="py-1 text-xs text-muted-foreground"
          style={{ paddingLeft: 4 + (parent.depth + 1) * 12 + 24 }}
        >
          (첫 {DOC_PAGE_SIZE}건만 표시)
        </li>
      )}
    </ul>
  );
}

/**
 * 컬렉션 경로를 `list_collection_doc_ids` 파라미터로 분해한다.
 * - "Hospital" → { collection_id: "Hospital", parent_path: undefined }
 * - "Hospital/abc/ActivityLog" → { collection_id: "ActivityLog", parent_path: "Hospital/abc" }
 */
function splitForList(path: string): {
  collection_id: string;
  parent_path?: string;
} {
  const segs = path.split("/").filter(Boolean);
  if (segs.length === 1) return { collection_id: segs[0] };
  const collection_id = segs[segs.length - 1];
  const parent_path = segs.slice(0, -1).join("/");
  return { collection_id, parent_path };
}
