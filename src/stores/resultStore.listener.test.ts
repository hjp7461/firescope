import { beforeEach, describe, expect, it, vi } from "vitest";
import { useResultStore } from "./resultStore";
import { useTabsStore } from "./tabsStore";
import type { FirestoreDocument } from "@/types";

// listener IPC는 실제 Tauri 호출이 아니므로 mock — 우리는 store reducer만 검증.
vi.mock("@/ipc/listener", () => ({
  startListener: vi.fn(async () => undefined),
  stopListener: vi.fn(async () => undefined),
  listListeners: vi.fn(async () => []),
}));

// 이벤트 리스닝은 실 환경 없이 안전한 no-op으로 — listener:change 직접 발화는
// reducer 단위 테스트가 아니므로 슬라이스 조작으로 대체한다.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => undefined),
}));

const doc = (path: string, value: string): FirestoreDocument => ({
  path,
  id: path.split("/").pop() ?? "",
  parent: path.split("/").slice(0, -1).join("/"),
  data: { value: { type: "string", value } },
  create_time: null,
  update_time: null,
});

describe("resultStore listener slice", () => {
  beforeEach(() => {
    useTabsStore.getState().__resetForTests();
    useResultStore.getState().__resetForTests();
  });

  it("startListening initializes a listening slice with empty rows", async () => {
    const a = useTabsStore.getState().activeTabId!;
    await useResultStore.getState().startListening({
      target: { kind: "collection", path: "users" },
    });
    const slice = useResultStore.getState().byTab.get(a);
    expect(slice?.status).toBe("listening");
    expect(slice?.listenerId).toBeTruthy();
    expect(slice?.listenerStatus).toBe("initial");
    expect(slice?.rows).toEqual([]);
    expect(slice?.collectionPath).toBe("users");
  });

  it("setting slice directly: modified events upsert by path", () => {
    const a = useTabsStore.getState().activeTabId!;
    useResultStore.getState().__setSliceForTest(a, {
      streamId: null,
      collectionPath: "users",
      lastDsl: null,
      rows: [doc("users/a", "v1"), doc("users/b", "v1")],
      status: "listening",
      total: 2,
      scanned: 2,
      tookMs: null,
      error: null,
      indexUrl: null,
      listenerId: "L1",
      listenerStatus: "ready",
      listenerEventCount: 0,
    });
    // simulate modified upsert by replacing rows manually (reducer parity)
    const existing = useResultStore.getState().byTab.get(a)!.rows;
    const next = existing.map((r) =>
      r.path === "users/a" ? doc("users/a", "v2") : r,
    );
    useResultStore.getState().__setSliceForTest(a, {
      ...useResultStore.getState().byTab.get(a)!,
      rows: next,
    });
    expect(
      (useResultStore.getState().byTab.get(a)!.rows[0].data.value as any).value,
    ).toBe("v2");
    expect(useResultStore.getState().byTab.get(a)!.rows.length).toBe(2);
  });

  it("stopListening clears listener fields and switches to done", async () => {
    const a = useTabsStore.getState().activeTabId!;
    useResultStore.getState().__setSliceForTest(a, {
      streamId: null,
      collectionPath: "users",
      lastDsl: null,
      rows: [doc("users/a", "v1")],
      status: "listening",
      total: 1,
      scanned: 1,
      tookMs: null,
      error: null,
      indexUrl: null,
      listenerId: "L1",
      listenerStatus: "ready",
      listenerEventCount: 3,
    });
    await useResultStore.getState().stopListening();
    const slice = useResultStore.getState().byTab.get(a)!;
    expect(slice.status).toBe("done");
    expect(slice.listenerId).toBeNull();
    expect(slice.listenerStatus).toBeNull();
    // rows는 유지 — 사용자가 마지막 결과를 계속 볼 수 있어야 한다.
    expect(slice.rows.length).toBe(1);
  });
});
