// Realtime 리스너 IPC 래퍼 (`docs/03-ipc-contract.md` §8.5 v0.10).

import { call } from "./index";
import { getActiveSessionId } from "@/stores/tabsStore";
import type { ListenerDsl, ListenerInfo } from "@/types";

function requireActiveSessionId(): string {
  const id = getActiveSessionId();
  if (!id) throw new Error("no active session — activate a profile first");
  return id;
}

/**
 * Realtime listener를 등록·시작한다. 결과는 `listener:change:<id>` /
 * `listener:status:<id>` 이벤트로 흐른다.
 */
export const startListener = (listenerId: string, dsl: ListenerDsl) =>
  call<void>("start_listener", {
    session_id: requireActiveSessionId(),
    params: { listener_id: listenerId, dsl },
  });

/** listener를 종료한다. 알 수 없는 id는 idempotent. */
export const stopListener = (listenerId: string) =>
  call<void>("stop_listener", { listener_id: listenerId });

/** 활성 listener 메타데이터 목록 (디버그/세션 종료 시 일괄 처리용). */
export const listListeners = () => call<ListenerInfo[]>("list_listeners", {});
