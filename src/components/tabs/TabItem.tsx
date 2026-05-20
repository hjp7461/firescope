import { ShieldAlert, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { ModeIcon } from "@/components/profile/mode";
import { useProfileStore } from "@/stores/profileStore";
import type { Tab } from "@/stores/tabsStore";

export function TabItem({
  tab,
  isActive,
  onFocus,
  onClose,
}: {
  tab: Tab;
  isActive: boolean;
  onFocus: () => void;
  onClose: () => void;
}) {
  const profiles = useProfileStore((s) => s.profiles);
  const profile = tab.session
    ? profiles.find((p) => p.id === tab.session!.profile_id)
    : tab.pendingProfileId
      ? profiles.find((p) => p.id === tab.pendingProfileId)
      : undefined;

  const isDormant = !tab.session && !!tab.pendingProfileId;
  const isEmpty = !tab.session && !tab.pendingProfileId;

  const rawLabel = tab.session?.profile_name ?? profile?.name ?? "새 탭";
  const label = isDormant ? `— ${rawLabel}` : rawLabel;

  return (
    <div
      role="tab"
      aria-selected={isActive}
      onClick={onFocus}
      onAuxClick={(e) => {
        // Middle-click closes (browser convention).
        if (e.button === 1) onClose();
      }}
      className={cn(
        "group flex h-8 cursor-pointer select-none items-center gap-1.5 border-r px-2.5 text-xs",
        "min-w-0 max-w-[200px]",
        isActive
          ? "border-b-transparent bg-background font-semibold text-foreground shadow-[inset_0_2px_0_0] shadow-primary/60"
          : "border-b border-b-border bg-muted/30 text-muted-foreground hover:bg-muted/50 hover:text-foreground/80",
        isEmpty && "italic",
      )}
    >
      {profile && (
        <span
          className={cn(
            "size-2 shrink-0 rounded-full border",
            isDormant && "opacity-50",
          )}
          style={{ backgroundColor: profile.color ?? "transparent" }}
          aria-hidden
        />
      )}
      {profile && (
        <ModeIcon
          mode={profile.mode}
          className={cn(
            "size-3 shrink-0",
            isDormant ? "text-muted-foreground/60" : "text-muted-foreground",
          )}
        />
      )}
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {profile?.read_only_warning && !isActive && (
        <ShieldAlert className="size-3 shrink-0 text-destructive/70" aria-hidden />
      )}
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          onClose();
        }}
        className={cn(
          "ml-0.5 inline-flex size-4 shrink-0 items-center justify-center rounded-sm",
          "transition-opacity hover:bg-muted-foreground/20",
          "opacity-0 group-hover:opacity-100",
          isActive && "opacity-40",
        )}
        aria-label={`탭 닫기: ${rawLabel}`}
      >
        <X className="size-3" />
      </button>
    </div>
  );
}
