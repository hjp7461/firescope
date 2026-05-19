import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { ProfileSidebar } from "@/components/profile/ProfileSidebar";
import { ProfileFormDialog } from "@/components/profile/ProfileFormDialog";
import { ProductionWarningModal } from "@/components/profile/ProductionWarningModal";
import { useProfileStore } from "@/stores/profileStore";
import { useSessionStore } from "@/stores/sessionStore";
import { deleteProfile, duplicateProfile } from "@/ipc/profile";
import { activateProfile, currentSession } from "@/ipc/session";
import { asAppError, type ProfileMeta, type Session } from "@/types";
import { CollectionsPanel } from "@/components/views/CollectionsPanel";
import { TableView } from "@/components/views/TableView";
import { TreeView } from "@/components/views/TreeView";
import { JsonView } from "@/components/views/JsonView";
import { LogView } from "@/components/views/LogView";
import { ResultBar } from "@/components/views/ResultBar";
import { useViewStore } from "@/stores/viewStore";

function ResultPane() {
  const view = useViewStore((s) => s.activeView);
  if (view === "tree") return <TreeView />;
  if (view === "json") return <JsonView />;
  if (view === "log") return <LogView />;
  return <TableView />;
}

function App() {
  const loadProfiles = useProfileStore((s) => s.load);
  const upsert = useProfileStore((s) => s.upsert);
  const removeById = useProfileStore((s) => s.removeById);
  const profiles = useProfileStore((s) => s.profiles);
  const session = useSessionStore((s) => s.current);
  const setCurrent = useSessionStore((s) => s.setCurrent);

  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<ProfileMeta | null>(null);
  const [pendingProd, setPendingProd] = useState<ProfileMeta | null>(null);

  // 진입: 프로파일 목록 + 기존 세션 복구.
  useEffect(() => {
    loadProfiles();
    currentSession()
      .then(setCurrent)
      .catch(() => setCurrent(null));
  }, [loadProfiles, setCurrent]);

  // 백엔드 이벤트 구독 (원칙 10: 상태는 이벤트로 자동 동기화).
  useEffect(() => {
    const uns = Promise.all([
      listen<ProfileMeta>("profile:updated", (e) => upsert(e.payload)),
      listen<{ profile_id: string }>("profile:deleted", (e) =>
        removeById(e.payload.profile_id),
      ),
      listen<Session>("profile:activated", (e) => setCurrent(e.payload)),
      listen<{ profile_id: string }>("profile:deactivated", () =>
        setCurrent(null),
      ),
      listen<{ profile_id: string; expires_at: string }>(
        "profile:token_refreshed",
        () => toast.info("액세스 토큰이 갱신되었습니다"),
      ),
    ]);
    return () => {
      uns.then((fns) => fns.forEach((f) => f()));
    };
  }, [upsert, removeById, setCurrent]);

  async function doActivate(p: ProfileMeta, confirmed: boolean) {
    try {
      await activateProfile(p.id, confirmed);
      toast.success(`${p.name} 활성화됨`);
    } catch (err) {
      const e = asAppError(err);
      if (e.kind === "confirmation_required") {
        setPendingProd(p);
        return;
      }
      toast.error(e.message);
    }
  }

  function onActivate(p: ProfileMeta) {
    if (p.require_confirmation) setPendingProd(p);
    else void doActivate(p, false);
  }

  async function onDelete(p: ProfileMeta) {
    try {
      await deleteProfile(p.id);
      toast.success(`${p.name} 삭제됨`);
    } catch (err) {
      toast.error(asAppError(err).message);
    }
  }

  async function onDuplicate(p: ProfileMeta) {
    const newName = window.prompt("새 프로파일 이름", `${p.name} (복사본)`);
    if (!newName) return;
    try {
      await duplicateProfile(p.id, newName);
      toast.success("복제되었습니다 (자격증명은 다시 입력 필요)");
    } catch (err) {
      toast.error(asAppError(err).message);
    }
  }

  function openCreate() {
    setEditing(null);
    setFormOpen(true);
  }
  function openEdit(p: ProfileMeta) {
    setEditing(p);
    setFormOpen(true);
  }

  const activeProfile = session
    ? profiles.find((p) => p.id === session.profile_id)
    : undefined;

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <ProfileSidebar
        onAdd={openCreate}
        onActivate={onActivate}
        onEdit={openEdit}
        onDuplicate={onDuplicate}
        onSetCredential={openEdit}
        onDelete={onDelete}
      />

      <main className="flex min-w-0 flex-1 flex-col">
        {activeProfile?.read_only_warning && (
          <div className="bg-destructive px-4 py-1.5 text-center text-xs font-medium text-white">
            운영 환경에 연결되어 있습니다 · 읽기 전용
          </div>
        )}

        {session ? (
          <div className="flex min-w-0 flex-1 overflow-hidden">
            <CollectionsPanel />
            <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
              <ResultBar
                projectId={session.project_id}
                mode={session.mode}
              />
              <div className="min-w-0 flex-1 overflow-hidden">
                <ResultPane />
              </div>
            </div>
          </div>
        ) : (
          <div className="flex flex-1 items-center justify-center p-8">
            <p className="text-sm text-muted-foreground">
              왼쪽에서 프로파일을 더블클릭해 활성화하세요.
            </p>
          </div>
        )}
      </main>

      <ProfileFormDialog
        open={formOpen}
        onOpenChange={setFormOpen}
        initial={editing}
      />

      <ProductionWarningModal
        open={!!pendingProd}
        profileName={pendingProd?.name ?? ""}
        projectId={pendingProd?.project_id ?? ""}
        onConfirm={() => {
          const p = pendingProd;
          setPendingProd(null);
          if (p) void doActivate(p, true);
        }}
        onCancel={() => setPendingProd(null)}
      />
    </div>
  );
}

export default App;
