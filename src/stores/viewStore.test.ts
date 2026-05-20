import { beforeEach, describe, expect, it } from "vitest";
import { useViewStore } from "./viewStore";
import { useTabsStore } from "./tabsStore";

describe("viewStore byTab", () => {
  beforeEach(() => {
    useTabsStore.getState().__resetForTests();
    useViewStore.getState().__resetForTests();
  });

  it("default activeView is table on top-level", () => {
    expect(useViewStore.getState().activeView).toBe("table");
  });

  it("setView updates active tab and switches with focus", () => {
    const a = useTabsStore.getState().tabs[0].id;
    useViewStore.getState().setView("tree");
    expect(useViewStore.getState().activeView).toBe("tree");

    const b = useTabsStore.getState().add();
    expect(useViewStore.getState().activeView).toBe("table"); // fresh tab

    useViewStore.getState().setView("json");
    expect(useViewStore.getState().activeView).toBe("json");

    useTabsStore.getState().focus(a);
    expect(useViewStore.getState().activeView).toBe("tree");

    useTabsStore.getState().focus(b);
    expect(useViewStore.getState().activeView).toBe("json");
  });
});
