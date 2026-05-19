// 글로벌 단축키 (Phase 6-F): 쿼리 실행/취소/프로파일 빠른 전환.
//
// 입력 필드에서의 동작:
// - Cmd/Ctrl+Enter는 항상 동작 (textarea/input 안에서도 실행 가능해야 함)
// - Esc는 항상 동작
// - Cmd/Ctrl+1..9 (프로파일 전환)은 항상 동작 (충돌 가능성 낮음)

export type HotkeyHandlers = {
  /** Cmd/Ctrl+Enter — 쿼리 실행 */
  onRun?: () => void;
  /** Esc — 스트림 취소 */
  onCancel?: () => void;
  /** Cmd/Ctrl+<n> (1..9) — 프로파일 n번째 전환 */
  onSelectProfile?: (index: number) => void;
};

/** 키 이벤트를 핸들러로 라우팅하는 순수 로직 (테스트 용이). */
export function dispatchHotkey(
  ev: Pick<
    KeyboardEvent,
    "key" | "metaKey" | "ctrlKey" | "isComposing" | "preventDefault"
  >,
  handlers: HotkeyHandlers,
): "run" | "cancel" | "select" | null {
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

  if (meta && /^[1-9]$/.test(ev.key) && handlers.onSelectProfile) {
    ev.preventDefault();
    handlers.onSelectProfile(Number(ev.key) - 1);
    return "select";
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
