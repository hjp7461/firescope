import { Table2, ListTree, Braces, ScrollText } from "lucide-react";
import { cn } from "@/lib/utils";
import { useViewStore, type ViewKind } from "@/stores/viewStore";

const TABS: { kind: ViewKind; label: string; Icon: typeof Table2 }[] = [
  { kind: "table", label: "Table", Icon: Table2 },
  { kind: "tree", label: "Tree", Icon: ListTree },
  { kind: "json", label: "JSON", Icon: Braces },
  { kind: "log", label: "Log", Icon: ScrollText },
];

export function ViewTabs() {
  const active = useViewStore((s) => s.activeView);
  const setView = useViewStore((s) => s.setView);
  return (
    <div className="flex items-center gap-1">
      {TABS.map(({ kind, label, Icon }) => (
        <button
          key={kind}
          type="button"
          onClick={() => setView(kind)}
          className={cn(
            "flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium",
            active === kind
              ? "bg-primary/10 text-primary"
              : "text-muted-foreground hover:bg-accent",
          )}
        >
          <Icon className="size-3.5" />
          {label}
        </button>
      ))}
    </div>
  );
}
