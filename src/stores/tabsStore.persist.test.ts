import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  useTabsStore,
  tabsToBundle,
  hydrateTabs,
  setPersistenceEnabled,
} from "./tabsStore";
import type { ProfileMeta, Session, TabBundle } from "@/types";

function makeProfile(over: Partial<ProfileMeta> = {}): ProfileMeta {
  return {
    id: "p-1",
    name: "test",
    project_id: "demo",
    mode: "emulator",
    require_confirmation: false,
    read_only_warning: false,
    has_credential: true,
    use_count: 0,
    created_at: new Date().toISOString(),
    ...over,
  } as ProfileMeta;
}

function makeSession(profile_id: string): Session {
  return {
    session_id: `sess-${profile_id}`,
    profile_id,
    profile_name: "test",
    project_id: "demo",
    mode: "emulator",
    activated_at: new Date().toISOString(),
  };
}

describe("tabsStore persistence", () => {
  beforeEach(() => {
    setPersistenceEnabled(false);
    useTabsStore.getState().__resetForTests();
  });

  describe("tabsToBundle", () => {
    it("serializes active session profile_id", () => {
      const tabId = useTabsStore.getState().tabs[0].id;
      useTabsStore.getState().setSession(tabId, makeSession("p-1"));
      const bundle = tabsToBundle();
      expect(bundle.version).toBe(1);
      expect(bundle.tabs).toEqual([
        { id: tabId, profile_id: "p-1", order: 0 },
      ]);
      expect(bundle.active_tab_id).toBe(tabId);
    });

    it("serializes pendingProfileId for dormant tab", () => {
      const tabId = useTabsStore.getState().tabs[0].id;
      useTabsStore.getState().setPendingProfileId(tabId, "p-prod");
      const bundle = tabsToBundle();
      expect(bundle.tabs[0].profile_id).toBe("p-prod");
    });

    it("serializes empty tab with no profile_id", () => {
      const bundle = tabsToBundle();
      expect(bundle.tabs[0].profile_id).toBeUndefined();
    });

    it("preserves order across multiple tabs", () => {
      const a = useTabsStore.getState().tabs[0].id;
      const b = useTabsStore.getState().add();
      const c = useTabsStore.getState().add();
      const bundle = tabsToBundle();
      expect(bundle.tabs.map((t) => t.id)).toEqual([a, b, c]);
      expect(bundle.tabs.map((t) => t.order)).toEqual([0, 1, 2]);
    });
  });

  describe("hydrateTabs", () => {
    it("empty bundle keeps the initial 1 empty tab", async () => {
      const empty: TabBundle = { version: 1, tabs: [] };
      const activate = vi.fn();
      const profilesById = new Map<string, ProfileMeta>();
      await hydrateTabs(empty, profilesById, activate);
      const s = useTabsStore.getState();
      expect(s.tabs.length).toBe(1);
      expect(s.tabs[0].session).toBeNull();
      expect(activate).not.toHaveBeenCalled();
    });

    it("activates non-production profile on restore", async () => {
      const profile = makeProfile({ id: "p-emu", require_confirmation: false });
      const bundle: TabBundle = {
        version: 1,
        tabs: [{ id: "tab-1", profile_id: "p-emu", order: 0 }],
        active_tab_id: "tab-1",
      };
      const activate = vi.fn().mockResolvedValue(makeSession("p-emu"));
      const profilesById = new Map([["p-emu", profile]]);
      await hydrateTabs(bundle, profilesById, activate);
      const s = useTabsStore.getState();
      expect(s.tabs.length).toBe(1);
      expect(s.tabs[0].id).toBe("tab-1");
      expect(s.tabs[0].session?.profile_id).toBe("p-emu");
      expect(s.activeTabId).toBe("tab-1");
      expect(activate).toHaveBeenCalledWith("p-emu");
    });

    it("keeps production profile dormant (pendingProfileId, no session)", async () => {
      const profile = makeProfile({ id: "p-prod", require_confirmation: true });
      const bundle: TabBundle = {
        version: 1,
        tabs: [{ id: "tab-prod", profile_id: "p-prod", order: 0 }],
        active_tab_id: "tab-prod",
      };
      const activate = vi.fn();
      const profilesById = new Map([["p-prod", profile]]);
      await hydrateTabs(bundle, profilesById, activate);
      const s = useTabsStore.getState();
      expect(s.tabs[0].session).toBeNull();
      expect(s.tabs[0].pendingProfileId).toBe("p-prod");
      expect(activate).not.toHaveBeenCalled();
    });

    it("missing profile yields empty tab (no session, no pendingProfileId)", async () => {
      const bundle: TabBundle = {
        version: 1,
        tabs: [{ id: "tab-x", profile_id: "p-gone", order: 0 }],
      };
      const activate = vi.fn();
      const profilesById = new Map<string, ProfileMeta>();
      await hydrateTabs(bundle, profilesById, activate);
      const s = useTabsStore.getState();
      expect(s.tabs[0].session).toBeNull();
      expect(s.tabs[0].pendingProfileId).toBeUndefined();
      expect(activate).not.toHaveBeenCalled();
    });

    it("activation failure leaves empty tab (caught)", async () => {
      const profile = makeProfile({ id: "p-emu", require_confirmation: false });
      const bundle: TabBundle = {
        version: 1,
        tabs: [{ id: "tab-1", profile_id: "p-emu", order: 0 }],
      };
      const activate = vi.fn().mockRejectedValue(new Error("credential gone"));
      const profilesById = new Map([["p-emu", profile]]);
      await hydrateTabs(bundle, profilesById, activate);
      const s = useTabsStore.getState();
      expect(s.tabs[0].session).toBeNull();
      expect(s.tabs[0].pendingProfileId).toBeUndefined();
    });

    it("restores multiple tabs preserving order", async () => {
      const pa = makeProfile({ id: "pa", require_confirmation: false });
      const pb = makeProfile({ id: "pb", require_confirmation: false });
      const bundle: TabBundle = {
        version: 1,
        tabs: [
          { id: "t1", profile_id: "pa", order: 0 },
          { id: "t2", profile_id: "pb", order: 1 },
        ],
        active_tab_id: "t2",
      };
      const activate = vi
        .fn()
        .mockImplementation((id: string) => Promise.resolve(makeSession(id)));
      const profilesById = new Map([["pa", pa], ["pb", pb]]);
      await hydrateTabs(bundle, profilesById, activate);
      const s = useTabsStore.getState();
      expect(s.tabs.map((t) => t.id)).toEqual(["t1", "t2"]);
      expect(s.activeTabId).toBe("t2");
    });
  });
});
