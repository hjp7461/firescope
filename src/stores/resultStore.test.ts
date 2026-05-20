import { beforeEach, describe, expect, it } from "vitest";
import { useResultStore } from "./resultStore";
import { useTabsStore } from "./tabsStore";

const EMPTY_DSL = {
  target: { kind: "collection" as const, path: "users" },
  limit: 100,
};

describe("resultStore byTab", () => {
  beforeEach(() => {
    useTabsStore.getState().__resetForTests();
    useResultStore.getState().__resetForTests();
  });

  it("default top-level mirror is idle", () => {
    const s = useResultStore.getState();
    expect(s.status).toBe("idle");
    expect(s.rows).toEqual([]);
  });

  it("setActiveTabSlice updates top-level mirror", () => {
    const a = useTabsStore.getState().activeTabId!;
    useResultStore.getState().__setSliceForTest(a, {
      streamId: "s1",
      collectionPath: "users",
      lastDsl: EMPTY_DSL,
      rows: [{ path: "u/1", data: {} } as any],
      status: "done",
      total: 1,
      scanned: 1,
      tookMs: 10,
      error: null,
      indexUrl: null,
      listenerId: null,
      listenerStatus: null,
      listenerEventCount: 0,
    });
    const s = useResultStore.getState();
    expect(s.rows.length).toBe(1);
    expect(s.status).toBe("done");
  });

  it("inactive tab slice does not leak into top-level mirror", () => {
    const b = useTabsStore.getState().add(); // creates and focuses tab B
    const a = useTabsStore.getState().tabs[0].id;
    // Set slice for tab A while tab B is active.
    useResultStore.getState().__setSliceForTest(a, {
      streamId: "s-a",
      collectionPath: "users",
      lastDsl: EMPTY_DSL,
      rows: [{ path: "u/1", data: {} } as any],
      status: "done",
      total: 1,
      scanned: 1,
      tookMs: 10,
      error: null,
      indexUrl: null,
      listenerId: null,
      listenerStatus: null,
      listenerEventCount: 0,
    });
    const s = useResultStore.getState();
    expect(s.rows).toEqual([]); // tab B (active) is empty
    expect(s.byTab.get(a)?.rows.length).toBe(1); // tab A's data is preserved
    // Focus tab A → mirror updates
    useTabsStore.getState().focus(a);
    expect(useResultStore.getState().rows.length).toBe(1);
    // Focus tab B again → mirror is empty
    useTabsStore.getState().focus(b);
    expect(useResultStore.getState().rows.length).toBe(0);
  });

  it("streamIdToTab routes events to the correct tab", () => {
    const a = useTabsStore.getState().activeTabId!;
    const b = useTabsStore.getState().add(); // now b is active
    useResultStore.getState().__registerStreamForTest("stream-a", a);
    useResultStore.getState().__registerStreamForTest("stream-b", b);
    expect(useResultStore.getState().__getTabForStream("stream-a")).toBe(a);
    expect(useResultStore.getState().__getTabForStream("stream-b")).toBe(b);
    expect(useResultStore.getState().__getTabForStream("unknown")).toBeUndefined();
  });
});
