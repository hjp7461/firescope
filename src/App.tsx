import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { ProfileSidebar } from "@/components/profile/ProfileSidebar";
import { ProfileFormDialog } from "@/components/profile/ProfileFormDialog";
import { ProductionWarningModal } from "@/components/profile/ProductionWarningModal";
import { useProfileStore } from "@/stores/profileStore";
import { useActiveSession, useTabsStore } from "@/stores/tabsStore";
import { deleteProfile, duplicateProfile } from "@/ipc/profile";
import { activateProfile, currentSession } from "@/ipc/session";
import { asAppError, type ProfileMeta, type Session } from "@/types";
import { CollectionsPanel } from "@/components/views/CollectionsPanel";
import { TableView } from "@/components/views/TableView";
import { TreeView } from "@/components/views/TreeView";
import { JsonView } from "@/components/views/JsonView";
import { LogView } from "@/components/views/LogView";
import { ResultBar } from "@/components/views/ResultBar";
import { QueryBuilder } from "@/components/query-builder/QueryBuilder";
import { useViewStore } from "@/stores/viewStore";
import { useHistoryStore } from "@/stores/historyStore";
import { startLogStream } from "@/stores/logStore";
import { initTheme } from "@/stores/themeStore";
import { ThemeToggle } from "@/components/ThemeToggle";
import { useResultStore } from "@/stores/resultStore";
import { useQueryStore } from "@/stores/queryStore";
import { bindGlobalHotkeys } from "@/lib/hotkeys";
import { toKoreanMessage } from "@/lib/errorMessages";
import { EmptyState } from "@/components/EmptyState";

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
  const session = useActiveSession();

  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<ProfileMeta | null>(null);
  const [pendingProd, setPendingProd] = useState<ProfileMeta | null>(null);
  const [builderOpen, setBuilderOpen] = useState(true);

  // 진입: 프로파일 목록 + 기존 세션 복구 + 로그 스트림 시작 + 테마 적용.
  useEffect(() => {
    loadProfiles();
    const tabId = useTabsStore.getState().activeTabId;
    currentSession()
      .then((s) => tabId && useTabsStore.getState().setSession(tabId, s))
      .catch(() => {});
    void startLogStream();
    return initTheme();
  }, [loadProfiles]);

  // 활성 프로파일이 바뀌면 그 프로파일의 쿼리 히스토리를 로드 (격리).
  useEffect(() => {
    void useHistoryStore.getState().load(session?.profile_id ?? null);
  }, [session?.profile_id]);

  // 글로벌 단축키 (Phase 6-F). 콜백을 ref로 받아 매번 새 클로저를 캡처하면서도
  // keydown 리스너는 한 번만 bind한다 (마운트/언마운트 비용 절감).
  const onSelectProfileRef = useRef<(idx: number) => void>(() => {});
  onSelectProfileRef.current = (idx: number) => {
    const list = useProfileStore.getState().profiles;
    const p = list[idx];
    if (!p) return;
    if (p.require_confirmation) setPendingProd(p);
    else void doActivate(p, false);
  };
  useEffect(() => {
    return bindGlobalHotkeys({
      onRun: () => {
        const result = useQueryStore.getState().build();
        if (!result.ok) {
          toast.error(result.error);
          return;
        }
        void useResultStore.getState().runDsl(result.dsl);
      },
      onCancel: () => {
        if (useResultStore.getState().status === "streaming") {
          void useResultStore.getState().cancel();
        }
      },
      onSelectProfile: (idx) => onSelectProfileRef.current(idx),
    });
  }, []);

  // 백엔드 이벤트 구독 (원칙 10: 상태는 이벤트로 자동 동기화).
  useEffect(() => {
    const uns = Promise.all([
      listen<ProfileMeta>("profile:updated", (e) => upsert(e.payload)),
      listen<{ profile_id: string }>("profile:deleted", (e) =>
        removeById(e.payload.profile_id),
      ),
      listen<Session>("profile:activated", (e) => {
        const tabsState = useTabsStore.getState();
        const target =
          tabsState.tabs.find((t) => t.session?.session_id === e.payload.session_id) ??
          tabsState.tabs.find((t) => t.id === tabsState.activeTabId);
        if (target) tabsState.setSession(target.id, e.payload);
      }),
      listen<{ session_id: string; profile_id: string }>("profile:deactivated", (e) => {
        const tabsState = useTabsStore.getState();
        const target = tabsState.tabs.find(
          (t) => t.session?.session_id === e.payload.session_id,
        );
        if (target) tabsState.setSession(target.id, null);
      }),
      listen<{ profile_id: string; expires_at: string }>(
        "profile:token_refreshed",
        () => toast.info("액세스 토큰이 갱신되었습니다"),
      ),
    ]);
    return () => {
      uns.then((fns) => fns.forEach((f) => f()));
    };
  }, [upsert, removeById]);

  async function doActivate(p: ProfileMeta, confirmed: boolean) {
    try {
      const newSession = await activateProfile(p.id, confirmed);
      // profile:activated 이벤트보다 먼저 즉시 attach — race-free 보장.
      const tabId = useTabsStore.getState().activeTabId;
      if (tabId) useTabsStore.getState().setSession(tabId, newSession);
      toast.success(`${p.name} 활성화됨`);
    } catch (err) {
      const e = asAppError(err);
      if (e.kind === "confirmation_required") {
        setPendingProd(p);
        return;
      }
      toast.error(toKoreanMessage(e));
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
      toast.error(toKoreanMessage(err));
    }
  }

  async function onDuplicate(p: ProfileMeta) {
    const newName = window.prompt("새 프로파일 이름", `${p.name} (복사본)`);
    if (!newName) return;
    try {
      await duplicateProfile(p.id, newName);
      toast.success("복제되었습니다 (자격증명은 다시 입력 필요)");
    } catch (err) {
      toast.error(toKoreanMessage(err));
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
          <div className="flex items-center justify-center gap-2 bg-destructive px-4 py-1.5 text-xs font-medium text-white">
            <span className="rounded-sm bg-white/20 px-1.5 py-0.5">운영</span>
            <span>{activeProfile.name}</span>
            <span className="opacity-80">·</span>
            <code className="opacity-90">{activeProfile.project_id}</code>
            <span className="opacity-80">·</span>
            <span>읽기 전용 (쓰기 요청 차단됨)</span>
          </div>
        )}
        <div className="absolute right-2 top-1 z-10">
          <ThemeToggle className="size-7 p-0" />
        </div>

        {session ? (
          <div className="flex min-w-0 flex-1 overflow-hidden">
            <CollectionsPanel />
            <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
              <ResultBar
                projectId={session.project_id}
                mode={session.mode}
                builderOpen={builderOpen}
                onToggleBuilder={() => setBuilderOpen((v) => !v)}
              />
              {builderOpen && <QueryBuilder />}
              <div className="min-w-0 flex-1 overflow-hidden">
                <ResultPane />
              </div>
            </div>
          </div>
        ) : (
          <EmptyState profileCount={profiles.length} onAdd={openCreate} />
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
