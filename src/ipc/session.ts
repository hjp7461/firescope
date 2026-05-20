// 세션 IPC 타입 안전 래퍼 (`docs/03-ipc-contract.md` §2).
// PR 2: tabsStore.activeSessionId가 권위. module-level shim 제거.

import { invoke } from "@tauri-apps/api/core";
import { asAppError } from "@/types";
import type { Session } from "@/types";
import { getActiveSessionId } from "@/stores/tabsStore";

/** invoke를 감싸 거부 값을 항상 `AppError`로 정규화해 throw 한다. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    throw asAppError(err);
  }
}

function requireActiveSessionId(): string {
  const id = getActiveSessionId();
  if (!id) throw new Error("no active session — call activateProfile first");
  return id;
}

export async function activateProfile(
  profile_id: string,
  confirmed: boolean,
  session_id: string | null = null,
): Promise<Session> {
  // session_id=null → 백엔드가 새 세션 생성.
  // PR 3은 기존 탭 자리 교체 시 활성 탭의 session_id 전달.
  return call<Session>("activate_profile", {
    profile_id,
    session_id,
    confirmed,
  });
}

export async function currentSession(
  session_id?: string,
): Promise<Session | null> {
  const sid = session_id ?? getActiveSessionId();
  if (!sid) return null;
  return call<Session | null>("current_session", { session_id: sid });
}

export async function listSessions(): Promise<Session[]> {
  return call<Session[]>("list_sessions");
}

export async function deactivate(session_id?: string): Promise<void> {
  const sid = session_id ?? getActiveSessionId();
  if (!sid) return;
  await call<void>("deactivate", { session_id: sid });
}

export async function refreshToken(
  session_id?: string,
): Promise<{ expires_at: string }> {
  const sid = session_id ?? requireActiveSessionId();
  return call<{ expires_at: string }>("refresh_token", { session_id: sid });
}
