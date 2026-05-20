import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";
import { TabBar } from "./TabBar";
import { useTabsStore } from "@/stores/tabsStore";

describe("TabBar", () => {
  beforeEach(() => {
    useTabsStore.getState().__resetForTests();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders one tab on initial state with '새 탭' label", () => {
    render(<TabBar />);
    expect(screen.getByText("새 탭")).toBeInTheDocument();
  });

  it("renders the new-tab '+' button and adds tabs on click", () => {
    render(<TabBar />);
    const addBtn = screen.getByLabelText("탭 추가");
    fireEvent.click(addBtn);
    expect(useTabsStore.getState().tabs.length).toBe(2);
  });

  it("clicking a non-active tab focuses it", () => {
    render(<TabBar />);
    const initialTab = useTabsStore.getState().tabs[0].id;
    useTabsStore.getState().add(); // tab B becomes active
    const items = screen.getAllByRole("tab");
    fireEvent.click(items[0]);
    expect(useTabsStore.getState().activeTabId).toBe(initialTab);
  });

  it("close button removes the tab", () => {
    render(<TabBar />);
    useTabsStore.getState().add();
    const beforeCount = useTabsStore.getState().tabs.length;
    const closeBtns = screen.getAllByLabelText(/탭 닫기/);
    fireEvent.click(closeBtns[0]);
    expect(useTabsStore.getState().tabs.length).toBe(beforeCount - 1);
  });
});
