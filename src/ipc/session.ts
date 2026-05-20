// 세션 IPC 타입 안전 래퍼 (`docs/03-ipc-contract.md` §2).
// PR 1 shim: module-level currentSessionId auto-injected into session-scoped commands.
// PR 2에서 tabsStore가 권위가 될 때 이 shim 제거 예정.

import { invoke } from "@tauri-apps/api/core";
import { asAppError } from "@/types";
import type { Session } from "@/types";

let currentSessionId: string | null = null;

/** PR 2에서 tabsStore가 권위가 될 때까지 임시 어댑터. */
export function setCurrentSessionId(id: string | null): void {
  currentSessionId = id;
}

/** Internal: PR 2 진입 전까지 session-scoped IPC가 사용. */
export function requireSessionIdForIpc(): string {
  if (!currentSessionId) {
    throw new Error("no active session — call activateProfile first");
  }
  return currentSessionId;
}

/** invoke를 감싸 거부 값을 항상 `AppError`로 정규화해 throw 한다. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    throw asAppError(err);
  }
}

export async function activateProfile(
  profile_id: string,
  confirmed?: boolean,
): Promise<Session> {
  // session_id=null → 백엔드가 새 세션 생성. 결과 session_id를 저장.
  const s = await call<Session>("activate_profile", {
    profile_id,
    session_id: null,
    confirmed: confirmed ?? false,
  });
  setCurrentSessionId(s.session_id);
  return s;
}

export async function currentSession(): Promise<Session | null> {
  if (!currentSessionId) return null;
  return call<Session | null>("current_session", { session_id: currentSessionId });
}

export async function listSessions(): Promise<Session[]> {
  return call<Session[]>("list_sessions");
}

export async function deactivate(): Promise<void> {
  if (!currentSessionId) return;
  await call<void>("deactivate", { session_id: currentSessionId });
  setCurrentSessionId(null);
}

export async function refreshToken(): Promise<{ expires_at: string }> {
  return call<{ expires_at: string }>("refresh_token", {
    session_id: requireSessionIdForIpc(),
  });
}
