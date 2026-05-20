import { Plus } from "lucide-react";
import { cn } from "@/lib/utils";
import { useTabsStore, type Tab } from "@/stores/tabsStore";
import { TabItem } from "./TabItem";

export function TabBar({
  className,
  onDormantClick,
}: {
  className?: string;
  onDormantClick?: (tab: Tab) => void;
}) {
  const tabs = useTabsStore((s) => s.tabs);
  const activeTabId = useTabsStore((s) => s.activeTabId);
  const focus = useTabsStore((s) => s.focus);
  const close = useTabsStore((s) => s.close);
  const add = useTabsStore((s) => s.add);

  return (
    <div
      role="tablist"
      className={cn(
        "flex h-9 items-stretch overflow-x-auto border-b bg-muted/20",
        className,
      )}
    >
      <div className="flex min-w-0 flex-1 items-stretch">
        {tabs.map((tab) => (
          <TabItem
            key={tab.id}
            tab={tab}
            isActive={tab.id === activeTabId}
            onFocus={() => focus(tab.id)}
            onClose={() => close(tab.id)}
            onDormantClick={
              onDormantClick ? () => onDormantClick(tab) : undefined
            }
          />
        ))}
      </div>
      <button
        type="button"
        onClick={add}
        aria-label="탭 추가"
        className="inline-flex size-9 shrink-0 items-center justify-center border-b border-l hover:bg-muted/40"
      >
        <Plus className="size-4" />
      </button>
    </div>
  );
}
