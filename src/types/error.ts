// Rust `AppError`의 직렬화 형태 (`#[serde(tag = "kind")]`).
// 모든 변형이 `message: string`을 갖는다 (`docs/03-ipc-contract.md`).

export type AppErrorKind =
  | "auth"
  | "firestore"
  | "invalid_query"
  | "io"
  | "internal"
  | "no_session"
  | "profile_not_found"
  | "credential_not_found"
  | "credential_invalid"
  | "confirmation_required"
  | "vault_error"
  | "duplicate_profile"
  | "session_not_found"
  | "session_limit_reached";

export type AppError =
  | {
      kind: Exclude<AppErrorKind, "session_not_found" | "session_limit_reached">;
      message: string;
    }
  | { kind: "session_not_found"; session_id: string; message: string }
  | { kind: "session_limit_reached"; active: number; max: number; message: string };

/** invoke 거부 값을 AppError로 정규화. 알 수 없는 형태는 internal로 감싼다. */
export function asAppError(err: unknown): AppError {
  if (
    err &&
    typeof err === "object" &&
    "kind" in err &&
    typeof (err as { kind: unknown }).kind === "string"
  ) {
    const e = err as Record<string, unknown>;
    const kind = e.kind as AppErrorKind;
    const message = typeof e.message === "string" ? e.message : "";
    if (kind === "session_not_found") {
      return {
        kind,
        message,
        session_id: typeof e.session_id === "string" ? e.session_id : "",
      };
    }
    if (kind === "session_limit_reached") {
      return {
        kind,
        message,
        active: typeof e.active === "number" ? e.active : 0,
        max: typeof e.max === "number" ? e.max : 0,
      };
    }
    return { kind, message };
  }
  return {
    kind: "internal",
    message: err instanceof Error ? err.message : String(err),
  };
}
