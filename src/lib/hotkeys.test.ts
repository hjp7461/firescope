import { describe, expect, it, vi } from "vitest";
import { dispatchHotkey } from "./hotkeys";

function ev(over: Partial<KeyboardEvent>): KeyboardEvent {
  return {
    key: "",
    metaKey: false,
    ctrlKey: false,
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
    expect(
      dispatchHotkey(ev({ key: "Enter", ctrlKey: true }), { onRun }),
    ).toBe("run");
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

  it("Cmd+1..9 selects profile index 0..8", () => {
    const onSelectProfile = vi.fn();
    for (let i = 1; i <= 9; i++) {
      onSelectProfile.mockClear();
      expect(
        dispatchHotkey(ev({ key: String(i), metaKey: true }), {
          onSelectProfile,
        }),
      ).toBe("select");
      expect(onSelectProfile).toHaveBeenCalledWith(i - 1);
    }
  });

  it("Cmd+0 does NOT select (only 1..9)", () => {
    const onSelectProfile = vi.fn();
    expect(
      dispatchHotkey(ev({ key: "0", metaKey: true }), { onSelectProfile }),
    ).toBeNull();
    expect(onSelectProfile).not.toHaveBeenCalled();
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
