import { beforeEach, describe, expect, it } from "vitest";
import { useTabsStore } from "./tabsStore";

describe("tabsStore", () => {
  beforeEach(() => {
    useTabsStore.getState().__resetForTests();
  });

  it("initializes with one empty tab", () => {
    const s = useTabsStore.getState();
    expect(s.tabs.length).toBe(1);
    expect(s.tabs[0].session).toBeNull();
    expect(s.tabs[0].pendingProfileId).toBeUndefined();
    expect(s.activeTabId).toBe(s.tabs[0].id);
  });

  it("add() creates a new empty tab and focuses it", () => {
    const before = useTabsStore.getState().tabs[0].id;
    const newId = useTabsStore.getState().add();
    const s = useTabsStore.getState();
    expect(s.tabs.length).toBe(2);
    expect(newId).not.toBe(before);
    expect(s.activeTabId).toBe(newId);
    expect(s.tabs[1].session).toBeNull();
  });

  it("close() removes a tab and refocuses to a sibling", () => {
    const a = useTabsStore.getState().tabs[0].id;
    const b = useTabsStore.getState().add();
    useTabsStore.getState().close(b);
    const s = useTabsStore.getState();
    expect(s.tabs.length).toBe(1);
    expect(s.activeTabId).toBe(a);
  });

  it("close() on the last tab leaves an auto-created empty tab", () => {
    const a = useTabsStore.getState().tabs[0].id;
    useTabsStore.getState().close(a);
    const s = useTabsStore.getState();
    expect(s.tabs.length).toBe(1);
    expect(s.tabs[0].id).not.toBe(a);
    expect(s.activeTabId).toBe(s.tabs[0].id);
  });

  it("setSession() attaches a session to a specific tab", () => {
    const tabId = useTabsStore.getState().tabs[0].id;
    const sess = {
      session_id: "s-1",
      profile_id: "p-1",
      profile_name: "x",
      project_id: "demo",
      mode: "emulator" as const,
      activated_at: new Date().toISOString(),
    };
    useTabsStore.getState().setSession(tabId, sess);
    const tab = useTabsStore.getState().tabs.find((t) => t.id === tabId);
    expect(tab?.session?.session_id).toBe("s-1");
    expect(tab?.pendingProfileId).toBeUndefined();
  });

  it("setSession(null) clears the session but keeps the tab", () => {
    const tabId = useTabsStore.getState().tabs[0].id;
    useTabsStore.getState().setSession(tabId, {
      session_id: "s-1",
      profile_id: "p-1",
      profile_name: "x",
      project_id: "demo",
      mode: "emulator" as const,
      activated_at: new Date().toISOString(),
    });
    useTabsStore.getState().setSession(tabId, null);
    const tab = useTabsStore.getState().tabs.find((t) => t.id === tabId);
    expect(tab?.session).toBeNull();
  });

  it("activeSessionId returns the active tab's session_id or null", () => {
    const tabId = useTabsStore.getState().tabs[0].id;
    expect(useTabsStore.getState().activeSessionId()).toBeNull();
    useTabsStore.getState().setSession(tabId, {
      session_id: "s-7",
      profile_id: "p-1",
      profile_name: "x",
      project_id: "demo",
      mode: "emulator" as const,
      activated_at: new Date().toISOString(),
    });
    expect(useTabsStore.getState().activeSessionId()).toBe("s-7");
  });
});
