import { describe, expect, it, vi } from "vitest";
import { dispatchHotkey } from "./hotkeys";

function ev(over: Partial<KeyboardEvent>): KeyboardEvent {
  return {
    key: "",
    metaKey: false,
    ctrlKey: false,
    shiftKey: false,
    isComposing: false,
    preventDefault: vi.fn(),
    ...over,
  } as unknown as KeyboardEvent;
}

describe("dispatchHotkey", () => {
  it("Cmd+Enter triggers onRun", () => {
    const onRun = vi.fn();
    const e = ev({ key: "Enter", metaKey: true });
    expect(dispatchHotkey(e, { onRun })).toBe("run");
    expect(onRun).toHaveBeenCalled();
    expect(e.preventDefault).toHaveBeenCalled();
  });

  it("Ctrl+Enter also triggers onRun (Windows/Linux)", () => {
    const onRun = vi.fn();
    expect(dispatchHotkey(ev({ key: "Enter", ctrlKey: true }), { onRun })).toBe(
      "run",
    );
    expect(onRun).toHaveBeenCalled();
  });

  it("Escape triggers onCancel", () => {
    const onCancel = vi.fn();
    expect(dispatchHotkey(ev({ key: "Escape" }), { onCancel })).toBe("cancel");
    expect(onCancel).toHaveBeenCalled();
  });

  it("Escape during IME composition is ignored", () => {
    const onCancel = vi.fn();
    expect(
      dispatchHotkey(ev({ key: "Escape", isComposing: true }), { onCancel }),
    ).toBeNull();
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("Cmd+1..9 selects tab index 0..8", () => {
    const onSelectTab = vi.fn();
    for (let i = 1; i <= 9; i++) {
      onSelectTab.mockClear();
      expect(
        dispatchHotkey(ev({ key: String(i), metaKey: true }), { onSelectTab }),
      ).toBe("select-tab");
      expect(onSelectTab).toHaveBeenCalledWith(i - 1);
    }
  });

  it("Cmd+0 does NOT select (only 1..9)", () => {
    const onSelectTab = vi.fn();
    expect(
      dispatchHotkey(ev({ key: "0", metaKey: true }), { onSelectTab }),
    ).toBeNull();
    expect(onSelectTab).not.toHaveBeenCalled();
  });

  it("Cmd+T triggers onNewTab", () => {
    const onNewTab = vi.fn();
    const e = ev({ key: "t", metaKey: true });
    expect(dispatchHotkey(e, { onNewTab })).toBe("new-tab");
    expect(onNewTab).toHaveBeenCalled();
    expect(e.preventDefault).toHaveBeenCalled();
  });

  it("Cmd+W triggers onCloseTab", () => {
    const onCloseTab = vi.fn();
    const e = ev({ key: "w", metaKey: true });
    expect(dispatchHotkey(e, { onCloseTab })).toBe("close-tab");
    expect(onCloseTab).toHaveBeenCalled();
    expect(e.preventDefault).toHaveBeenCalled();
  });

  it("Ctrl+Tab triggers onNextTab", () => {
    const onNextTab = vi.fn();
    const e = ev({ key: "Tab", ctrlKey: true });
    expect(dispatchHotkey(e, { onNextTab })).toBe("next-tab");
    expect(onNextTab).toHaveBeenCalled();
    expect(e.preventDefault).toHaveBeenCalled();
  });

  it("Ctrl+Shift+Tab triggers onPrevTab", () => {
    const onPrevTab = vi.fn();
    const e = ev({ key: "Tab", ctrlKey: true, shiftKey: true });
    expect(dispatchHotkey(e, { onPrevTab })).toBe("prev-tab");
    expect(onPrevTab).toHaveBeenCalled();
    expect(e.preventDefault).toHaveBeenCalled();
  });

  it("plain Enter without modifier does nothing", () => {
    const onRun = vi.fn();
    expect(dispatchHotkey(ev({ key: "Enter" }), { onRun })).toBeNull();
    expect(onRun).not.toHaveBeenCalled();
  });

  it("returns null when no handler is provided", () => {
    expect(dispatchHotkey(ev({ key: "Enter", metaKey: true }), {})).toBeNull();
  });
});
