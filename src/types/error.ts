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
  | "duplicate_profile";

export type AppError = {
  kind: AppErrorKind;
  message: string;
};

/** invoke 거부 값을 AppError로 정규화. 알 수 없는 형태는 internal로 감싼다. */
export function asAppError(err: unknown): AppError {
  if (
    err &&
    typeof err === "object" &&
    "kind" in err &&
    typeof (err as { kind: unknown }).kind === "string"
  ) {
    const e = err as { kind: string; message?: unknown };
    return {
      kind: e.kind as AppErrorKind,
      message: typeof e.message === "string" ? e.message : "",
    };
  }
  return {
    kind: "internal",
    message: err instanceof Error ? err.message : String(err),
  };
}
