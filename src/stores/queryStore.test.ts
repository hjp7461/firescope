import { beforeEach, describe, expect, it } from "vitest";
import { useQueryStore } from "./queryStore";
import { useTabsStore } from "./tabsStore";

describe("queryStore byTab", () => {
  beforeEach(() => {
    useTabsStore.getState().__resetForTests();
    useQueryStore.getState().__resetForTests();
  });

  it("starts with empty draft on top-level mirror", () => {
    const s = useQueryStore.getState();
    expect(s.target).toBe("");
    expect(s.wheres).toEqual([]);
  });

  it("setTarget updates the active tab slice and top-level mirror", () => {
    useQueryStore.getState().setTarget("users");
    expect(useQueryStore.getState().target).toBe("users");
  });

  it("switching tabs swaps the draft", () => {
    const a = useTabsStore.getState().tabs[0].id;
    useQueryStore.getState().setTarget("users"); // sets tab A's draft

    const b = useTabsStore.getState().add(); // creates tab B, focuses it
    expect(useQueryStore.getState().target).toBe("");
    useQueryStore.getState().setTarget("orders");
    expect(useQueryStore.getState().target).toBe("orders");

    useTabsStore.getState().focus(a);
    expect(useQueryStore.getState().target).toBe("users");

    useTabsStore.getState().focus(b);
    expect(useQueryStore.getState().target).toBe("orders");
  });
});
