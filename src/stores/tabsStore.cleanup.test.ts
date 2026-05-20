import { beforeEach, describe, expect, it } from "vitest";
import { useTabsStore } from "./tabsStore";
import { useResultStore } from "./resultStore";
import { useQueryStore } from "./queryStore";
import { useViewStore } from "./viewStore";

const EMPTY_DSL = {
  target: { kind: "collection" as const, path: "users" },
  limit: 100,
};

describe("tabsStore close → store cleanup", () => {
  beforeEach(() => {
    useTabsStore.getState().__resetForTests();
    useResultStore.getState().__resetForTests();
    useQueryStore.getState().__resetForTests();
    useViewStore.getState().__resetForTests();
  });

  it("close(id) drops the closed tab's slice from resultStore", () => {
    const a = useTabsStore.getState().tabs[0].id;
    const b = useTabsStore.getState().add();
    useResultStore.getState().__setSliceForTest(b, {
      streamId: null,
      collectionPath: "orders",
      lastDsl: EMPTY_DSL,
      rows: [{ path: "o/1", data: {} } as any],
      status: "done",
      total: 1,
      scanned: 1,
      tookMs: 0,
      error: null,
      indexUrl: null,
    });
    expect(useResultStore.getState().byTab.has(b)).toBe(true);
    useTabsStore.getState().close(b);
    expect(useResultStore.getState().byTab.has(b)).toBe(false);
    expect(useTabsStore.getState().activeTabId).toBe(a);
  });

  it("close(id) drops the closed tab's slice from queryStore", () => {
    const b = useTabsStore.getState().add();
    useQueryStore.getState().setTarget("orders");
    expect(useQueryStore.getState().byTab.has(b)).toBe(true);
    useTabsStore.getState().close(b);
    expect(useQueryStore.getState().byTab.has(b)).toBe(false);
  });

  it("close(id) drops the closed tab's slice from viewStore", () => {
    const b = useTabsStore.getState().add();
    useViewStore.getState().setView("tree");
    expect(useViewStore.getState().byTab.has(b)).toBe(true);
    useTabsStore.getState().close(b);
    expect(useViewStore.getState().byTab.has(b)).toBe(false);
  });

  it("close(id) drops streamIdToTab entries owned by that tab", () => {
    const b = useTabsStore.getState().add();
    useResultStore.getState().__registerStreamForTest("stream-x", b);
    expect(useResultStore.getState().__getTabForStream("stream-x")).toBe(b);
    useTabsStore.getState().close(b);
    expect(useResultStore.getState().__getTabForStream("stream-x")).toBeUndefined();
  });

  it("closing the last tab creates a fresh empty tab and cleans up", () => {
    const a = useTabsStore.getState().tabs[0].id;
    useResultStore.getState().__setSliceForTest(a, {
      streamId: null,
      collectionPath: null,
      lastDsl: null,
      rows: [],
      status: "idle",
      total: 0,
      scanned: 0,
      tookMs: null,
      error: null,
      indexUrl: null,
    });
    expect(useResultStore.getState().byTab.has(a)).toBe(true);
    useTabsStore.getState().close(a);
    expect(useResultStore.getState().byTab.has(a)).toBe(false);
    expect(useTabsStore.getState().tabs.length).toBe(1);
    expect(useTabsStore.getState().tabs[0].id).not.toBe(a);
  });
});
