import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { ProfileSidebar } from "@/components/profile/ProfileSidebar";
import { ProfileFormDialog } from "@/components/profile/ProfileFormDialog";
import { ProductionWarningModal } from "@/components/profile/ProductionWarningModal";
import { useProfileStore } from "@/stores/profileStore";
import { useActiveSession, useTabsStore, hydrateTabs, setPersistenceEnabled } from "@/stores/tabsStore";
import { deleteProfile, duplicateProfile } from "@/ipc/profile";
import { activateProfile } from "@/ipc/session";
import { listTabs } from "@/ipc/tabs";
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
import { TabBar } from "@/components/tabs/TabBar";
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
  const [pendingProd, setPendingProd] = useState<
    { profile: ProfileMeta; inNewTab: boolean } | null
  >(null);
  const [builderOpen, setBuilderOpen] = useState(true);

  // 진입: 프로파일 목록 + 탭 하이드레이트 + 로그 스트림 시작 + 테마 적용.
  useEffect(() => {
    let cleanup: (() => void) | undefined;
    (async () => {
      // 1) 프로파일 먼저 로드 — hydrate가 require_confirmation 플래그를 읽음
      await loadProfiles();
      // 2) 탭 하이드레이트 (이 단계 동안 persistence는 꺼둠)
      setPersistenceEnabled(false);
      try {
        const bundle = await listTabs();
        const profilesById = new Map(
          useProfileStore.getState().profiles.map((p) => [p.id, p]),
        );
        await hydrateTabs(bundle, profilesById, async (profile_id) => {
          return activateProfile(profile_id, false, null);
        });
      } catch (err) {
        toast.error(toKoreanMessage(err));
      } finally {
        setPersistenceEnabled(true);
      }
      // 3) 로그 스트림 + 테마
      void startLogStream();
      cleanup = initTheme();
    })();
    return () => {
      cleanup?.();
    };
  }, [loadProfiles]);

  // 활성 프로파일이 바뀌면 그 프로파일의 쿼리 히스토리를 로드 (격리).
  useEffect(() => {
    void useHistoryStore.getState().load(session?.profile_id ?? null);
  }, [session?.profile_id]);

  // 글로벌 단축키. 핸들러 내에서 getState()로 매번 최신 상태를 읽으므로 ref 불필요.
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
      onSelectTab: (idx) => {
        const tabs = useTabsStore.getState().tabs;
        const target = tabs[idx];
        if (target) useTabsStore.getState().focus(target.id);
      },
      onNewTab: () => {
        useTabsStore.getState().add();
      },
      onCloseTab: () => {
        const { activeTabId } = useTabsStore.getState();
        if (activeTabId) useTabsStore.getState().close(activeTabId);
      },
      onNextTab: () => {
        const { tabs, activeTabId } = useTabsStore.getState();
        if (tabs.length < 2) return;
        const i = tabs.findIndex((t) => t.id === activeTabId);
        const next = tabs[(i + 1) % tabs.length];
        useTabsStore.getState().focus(next.id);
      },
      onPrevTab: () => {
        const { tabs, activeTabId } = useTabsStore.getState();
        if (tabs.length < 2) return;
        const i = tabs.findIndex((t) => t.id === activeTabId);
        const prev = tabs[(i - 1 + tabs.length) % tabs.length];
        useTabsStore.getState().focus(prev.id);
      },
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
      listen<{ active: number; max: number }>("session:limit_warning", (e) => {
        toast.warning(
          `활성 세션 ${e.payload.active}/${e.payload.max} — 리소스 사용량이 늘어날 수 있습니다.`,
        );
      }),
    ]);
    return () => {
      uns.then((fns) => fns.forEach((f) => f()));
    };
  }, [upsert, removeById]);

  async function doActivate(p: ProfileMeta, confirmed: boolean, inNewTab: boolean) {
    try {
      let targetTabId: string | null;
      if (inNewTab) {
        targetTabId = useTabsStore.getState().add();
      } else {
        targetTabId = useTabsStore.getState().activeTabId;
      }
      // 활성 탭 자리 교체면 그 탭의 기존 session_id를 백엔드에 넘김 (없으면 null = 새 세션)
      const existingSessionId = inNewTab
        ? null
        : (useTabsStore.getState().tabs.find((t) => t.id === targetTabId)?.session?.session_id ?? null);
      const newSession = await activateProfile(p.id, confirmed, existingSessionId);
      if (targetTabId) {
        useTabsStore.getState().setSession(targetTabId, newSession);
      }
      toast.success(`${p.name} 활성화됨`);
    } catch (err) {
      const e = asAppError(err);
      if (e.kind === "confirmation_required") {
        setPendingProd({ profile: p, inNewTab });
        return;
      }
      toast.error(toKoreanMessage(e));
    }
  }

  function onActivate(p: ProfileMeta, inNewTab: boolean) {
    if (p.require_confirmation) {
      setPendingProd({ profile: p, inNewTab });
    } else {
      void doActivate(p, false, inNewTab);
    }
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
        {profiles.length > 0 && (
          <TabBar
            onDormantClick={(tab) => {
              if (!tab.pendingProfileId) return;
              const profile = profiles.find((p) => p.id === tab.pendingProfileId);
              if (!profile) return;
              // 활성화 전에 그 탭으로 포커스 — onActivate가 setSession 시 활성 탭에 attach
              useTabsStore.getState().focus(tab.id);
              onActivate(profile, false);
            }}
          />
        )}
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
        <div className="absolute right-2 top-10 z-10">
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
        profileName={pendingProd?.profile.name ?? ""}
        projectId={pendingProd?.profile.project_id ?? ""}
        onConfirm={() => {
          const p = pendingProd;
          setPendingProd(null);
          if (p) void doActivate(p.profile, true, p.inNewTab);
        }}
        onCancel={() => setPendingProd(null)}
      />
    </div>
  );
}

export default App;
