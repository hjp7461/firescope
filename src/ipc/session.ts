// 세션 IPC 타입 안전 래퍼 (`docs/03-ipc-contract.md` §2).

import { call } from "./index";
import type { Session } from "@/types";

export const activateProfile = (profileId: string, confirmed?: boolean) =>
  call<Session>("activate_profile", {
    profile_id: profileId,
    confirmed: confirmed ?? false,
  });

export const currentSession = () =>
  call<Session | null>("current_session");

export const deactivate = () => call<void>("deactivate");

export const refreshToken = () =>
  call<{ expires_at: string }>("refresh_token");
