import { Plus, ShieldAlert } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useProfileStore } from "@/stores/profileStore";
import { useSessionStore } from "@/stores/sessionStore";
import type { ProfileMeta } from "@/types";
import { ModeIcon } from "./mode";
import { ProfileContextMenu } from "./ProfileContextMenu";

export function ProfileSidebar({
  onAdd,
  onActivate,
  onEdit,
  onDuplicate,
  onSetCredential,
  onDelete,
}: {
  onAdd: () => void;
  onActivate: (p: ProfileMeta) => void;
  onEdit: (p: ProfileMeta) => void;
  onDuplicate: (p: ProfileMeta) => void;
  onSetCredential: (p: ProfileMeta) => void;
  onDelete: (p: ProfileMeta) => void;
}) {
  const profiles = useProfileStore((s) => s.profiles);
  const loading = useProfileStore((s) => s.loading);
  const activeProfileId = useSessionStore((s) => s.current?.profile_id ?? null);

  return (
    <aside className="flex h-full w-72 flex-col border-r bg-sidebar text-sidebar-foreground">
      <div className="flex items-center justify-between px-3 py-2.5">
        <span className="text-sm font-semibold">프로파일</span>
        <Button size="sm" variant="ghost" onClick={onAdd} aria-label="프로파일 추가">
          <Plus className="size-4" />
        </Button>
      </div>

      <ScrollArea className="flex-1 px-2">
        {loading && profiles.length === 0 ? (
          <p className="px-2 py-4 text-xs text-muted-foreground">불러오는 중…</p>
        ) : profiles.length === 0 ? (
          <div className="px-2 py-8 text-center">
            <p className="text-sm text-muted-foreground">
              등록된 프로파일이 없습니다.
            </p>
            <Button size="sm" className="mt-3" onClick={onAdd}>
              <Plus className="mr-1 size-4" />
              프로파일 추가하기
            </Button>
          </div>
        ) : (
          <ul className="space-y-0.5 pb-2">
            {profiles.map((p) => {
              const active = p.id === activeProfileId;
              return (
                <li key={p.id}>
                  <ProfileContextMenu
                    onEdit={() => onEdit(p)}
                    onDuplicate={() => onDuplicate(p)}
                    onSetCredential={() => onSetCredential(p)}
                    onDelete={() => onDelete(p)}
                  >
                    <button
                      type="button"
                      onDoubleClick={() => onActivate(p)}
                      className={cn(
                        "flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-sm transition-colors",
                        active
                          ? "bg-sidebar-accent text-sidebar-accent-foreground"
                          : "hover:bg-sidebar-accent/50",
                      )}
                      title="더블클릭하여 활성화"
                    >
                      <span
                        className="size-2.5 shrink-0 rounded-full border"
                        style={{ backgroundColor: p.color ?? "transparent" }}
                        aria-hidden
                      />
                      <ModeIcon
                        mode={p.mode}
                        className="size-4 shrink-0 text-muted-foreground"
                      />
                      <span className="min-w-0 flex-1">
                        <span className="flex items-center gap-1">
                          <span className="truncate font-medium">{p.name}</span>
                          {p.read_only_warning && (
                            <ShieldAlert className="size-3 shrink-0 text-destructive" />
                          )}
                        </span>
                        <span className="block truncate text-xs text-muted-foreground">
                          {p.project_id}
                        </span>
                      </span>
                      {!p.has_credential && p.mode !== "emulator" && (
                        <span className="shrink-0 text-[10px] text-amber-600">
                          자격증명 없음
                        </span>
                      )}
                    </button>
                  </ProfileContextMenu>
                </li>
              );
            })}
          </ul>
        )}
      </ScrollArea>

      {profiles.length > 0 && (
        <p className="border-t px-3 py-2 text-[11px] text-muted-foreground">
          더블클릭으로 활성화 · 우클릭으로 메뉴
        </p>
      )}
    </aside>
  );
}
