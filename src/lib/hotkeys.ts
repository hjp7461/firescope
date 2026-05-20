// 글로벌 단축키.
// 입력 필드에서의 동작:
// - Cmd/Ctrl+Enter는 항상 동작 (textarea/input 안에서도 실행 가능해야 함)
// - Esc는 항상 동작
// - Cmd/Ctrl+1..9, Cmd/Ctrl+T/W, Ctrl+Tab 도 항상 동작 (충돌 가능성 낮음)

export type HotkeyHandlers = {
  /** Cmd/Ctrl+Enter — 쿼리 실행 */
  onRun?: () => void;
  /** Esc — 스트림 취소 */
  onCancel?: () => void;
  /** Cmd/Ctrl+<n> (1..9) — n번째 탭으로 전환 */
  onSelectTab?: (index: number) => void;
  /** Cmd/Ctrl+T — 새 빈 탭 */
  onNewTab?: () => void;
  /** Cmd/Ctrl+W — 활성 탭 닫기 */
  onCloseTab?: () => void;
  /** Ctrl+Tab — 다음 탭 */
  onNextTab?: () => void;
  /** Ctrl+Shift+Tab — 이전 탭 */
  onPrevTab?: () => void;
};

export type HotkeyResult =
  | "run"
  | "cancel"
  | "select-tab"
  | "new-tab"
  | "close-tab"
  | "next-tab"
  | "prev-tab"
  | null;

/** 키 이벤트를 핸들러로 라우팅하는 순수 로직 (테스트 용이). */
export function dispatchHotkey(
  ev: Pick<
    KeyboardEvent,
    "key" | "metaKey" | "ctrlKey" | "shiftKey" | "isComposing" | "preventDefault"
  >,
  handlers: HotkeyHandlers,
): HotkeyResult {
  const meta = ev.metaKey || ev.ctrlKey;

  if (meta && ev.key === "Enter" && handlers.onRun) {
    ev.preventDefault();
    handlers.onRun();
    return "run";
  }

  if (ev.key === "Escape" && !ev.isComposing && handlers.onCancel) {
    handlers.onCancel();
    return "cancel";
  }

  // Ctrl+Shift+Tab before Ctrl+Tab so the shifted form wins.
  if (ev.ctrlKey && ev.key === "Tab" && ev.shiftKey && handlers.onPrevTab) {
    ev.preventDefault();
    handlers.onPrevTab();
    return "prev-tab";
  }

  if (ev.ctrlKey && ev.key === "Tab" && handlers.onNextTab) {
    ev.preventDefault();
    handlers.onNextTab();
    return "next-tab";
  }

  if (meta && (ev.key === "t" || ev.key === "T") && handlers.onNewTab) {
    ev.preventDefault();
    handlers.onNewTab();
    return "new-tab";
  }

  if (meta && (ev.key === "w" || ev.key === "W") && handlers.onCloseTab) {
    ev.preventDefault();
    handlers.onCloseTab();
    return "close-tab";
  }

  if (meta && /^[1-9]$/.test(ev.key) && handlers.onSelectTab) {
    ev.preventDefault();
    handlers.onSelectTab(Number(ev.key) - 1);
    return "select-tab";
  }

  return null;
}

export function bindGlobalHotkeys(handlers: HotkeyHandlers): () => void {
  const handler = (ev: KeyboardEvent) => {
    dispatchHotkey(ev, handlers);
  };
  window.addEventListener("keydown", handler);
  return () => window.removeEventListener("keydown", handler);
}
