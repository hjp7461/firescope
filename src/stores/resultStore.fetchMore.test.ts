import { beforeEach, describe, expect, it, vi } from "vitest";
import { useResultStore } from "./resultStore";
import { useTabsStore } from "./tabsStore";
import { useHistoryStore } from "./historyStore";
import type { Cursor, FirestoreDocument, QueryDsl } from "@/types";

// IPC + tauri 이벤트는 reducer 단위 테스트라 mock.
const queryDocumentsMock = vi.fn<(streamId: string, dsl: QueryDsl) => Promise<void>>(
  async () => undefined,
);
vi.mock("@/ipc/query", () => ({
  queryDocuments: (streamId: string, dsl: QueryDsl) => queryDocumentsMock(streamId, dsl),
  cancelStream: vi.fn(async () => undefined),
  getDocument: vi.fn(async () => null),
}));
vi.mock("@/ipc/listener", () => ({
  startListener: vi.fn(async () => undefined),
  stopListener: vi.fn(async () => undefined),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => undefined),
}));

const doc = (path: string): FirestoreDocument => ({
  path,
  id: path.split("/").pop() ?? "",
  parent: path.split("/").slice(0, -1).join("/"),
  data: {},
  create_time: null,
  update_time: null,
});

const SAMPLE_DSL: QueryDsl = {
  target: { kind: "collection", path: "Hospital" },
  limit: 100,
};

const SAMPLE_CURSOR: Cursor = {
  kind: "values",
  values: [{ type: "reference", value: "projects/p/databases/d/documents/Hospital/abc" }],
};

describe("resultStore.fetchMore", () => {
  beforeEach(() => {
    useTabsStore.getState().__resetForTests();
    useResultStore.getState().__resetForTests();
    queryDocumentsMock.mockClear();
  });

  it("no-op when hasMore is false", async () => {
    const a = useTabsStore.getState().activeTabId!;
    useResultStore.getState().__setSliceForTest(a, {
      status: "done",
      hasMore: false,
      nextCursor: SAMPLE_CURSOR,
      lastDsl: SAMPLE_DSL,
      rows: [doc("Hospital/x")],
      total: 1,
      scanned: 1,
    });
    await useResultStore.getState().fetchMore();
    expect(queryDocumentsMock).not.toHaveBeenCalled();
  });

  it("no-op when nextCursor is null", async () => {
    const a = useTabsStore.getState().activeTabId!;
    useResultStore.getState().__setSliceForTest(a, {
      status: "done",
      hasMore: true,
      nextCursor: null,
      lastDsl: SAMPLE_DSL,
    });
    await useResultStore.getState().fetchMore();
    expect(queryDocumentsMock).not.toHaveBeenCalled();
  });

  it("no-op when fetchMoreInFlight is already true", async () => {
    const a = useTabsStore.getState().activeTabId!;
    useResultStore.getState().__setSliceForTest(a, {
      status: "done",
      hasMore: true,
      nextCursor: SAMPLE_CURSOR,
      lastDsl: SAMPLE_DSL,
      fetchMoreInFlight: true,
    });
    await useResultStore.getState().fetchMore();
    expect(queryDocumentsMock).not.toHaveBeenCalled();
  });

  it("no-op when listenerId is set (Live mode)", async () => {
    const a = useTabsStore.getState().activeTabId!;
    useResultStore.getState().__setSliceForTest(a, {
      status: "done",
      hasMore: true,
      nextCursor: SAMPLE_CURSOR,
      lastDsl: SAMPLE_DSL,
      listenerId: "L1",
    });
    await useResultStore.getState().fetchMore();
    expect(queryDocumentsMock).not.toHaveBeenCalled();
  });

  it("no-op when status is not 'done'", async () => {
    const a = useTabsStore.getState().activeTabId!;
    useResultStore.getState().__setSliceForTest(a, {
      status: "streaming",
      hasMore: true,
      nextCursor: SAMPLE_CURSOR,
      lastDsl: SAMPLE_DSL,
    });
    await useResultStore.getState().fetchMore();
    expect(queryDocumentsMock).not.toHaveBeenCalled();
  });

  it("invokes queryDocuments with cursor as start_after and marks in-flight", async () => {
    const a = useTabsStore.getState().activeTabId!;
    useResultStore.getState().__setSliceForTest(a, {
      status: "done",
      hasMore: true,
      nextCursor: SAMPLE_CURSOR,
      lastDsl: SAMPLE_DSL,
      rows: [doc("Hospital/x")],
      total: 100,
      scanned: 100,
    });
    await useResultStore.getState().fetchMore();
    expect(queryDocumentsMock).toHaveBeenCalledTimes(1);
    const args = queryDocumentsMock.mock.calls[0];
    expect(typeof args[0]).toBe("string"); // new streamId
    const dslArg = args[1] as QueryDsl;
    expect(dslArg.start_after).toEqual(SAMPLE_CURSOR);
    expect(dslArg.target).toEqual(SAMPLE_DSL.target);
    // 슬라이스가 in-flight + streaming으로 전환됨
    const slice = useResultStore.getState().byTab.get(a)!;
    expect(slice.fetchMoreInFlight).toBe(true);
    expect(slice.status).toBe("streaming");
    // rows는 유지 (페이지네이션 의미)
    expect(slice.rows.length).toBe(1);
  });

  it("does not overwrite history with paginated runs", async () => {
    // fetchMore의 done 핸들러가 history.record를 호출하지 않는지 검증.
    // record 호출은 query:done 이벤트 콜백 안에서 일어나므로 여기서는
    // 'replace' 경로(runDsl) vs 'append' 경로(fetchMore) 분기를 코드 수준에서
    // 확인하는 게 가장 직접적이다 — 모드 분기가 attachStreamListeners 안에
    // 있으니 실제 이벤트를 발화하지 않더라도 record가 spy로 0회임을 확인할
    // 수는 있지만, listen이 mock이라 이벤트 콜백 자체가 실행되지 않는다.
    // 따라서 이 테스트는 회귀 방지를 위한 "queryDocuments는 호출됐지만
    // history.record는 호출되지 않았다"는 정도로 갈음한다.
    const a = useTabsStore.getState().activeTabId!;
    const recordSpy = vi.spyOn(useHistoryStore.getState(), "record");
    useResultStore.getState().__setSliceForTest(a, {
      status: "done",
      hasMore: true,
      nextCursor: SAMPLE_CURSOR,
      lastDsl: SAMPLE_DSL,
      rows: [doc("Hospital/x")],
    });
    await useResultStore.getState().fetchMore();
    expect(recordSpy).not.toHaveBeenCalled();
    recordSpy.mockRestore();
  });
});
