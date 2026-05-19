import { invoke } from "@tauri-apps/api/core";
import { asAppError } from "@/types";

/** invoke를 감싸 거부 값을 항상 `AppError`로 정규화해 throw 한다. */
export async function call<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    throw asAppError(err);
  }
}
