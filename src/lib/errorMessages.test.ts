import { describe, expect, it } from "vitest";
import { toKoreanMessage, toToastError } from "./errorMessages";

describe("toKoreanMessage", () => {
  it("AppError.kind마다 한국어 메시지 반환", () => {
    expect(toKoreanMessage({ kind: "no_session", message: "x" })).toMatch(
      /활성 세션/,
    );
    expect(toKoreanMessage({ kind: "duplicate_profile", message: "x" })).toMatch(
      /이미 있습니다/,
    );
    expect(toKoreanMessage({ kind: "credential_invalid", message: "x" })).toMatch(
      /자격증명/,
    );
  });

  it("AppError가 아닌 값도 internal로 정규화", () => {
    expect(toKoreanMessage(new Error("boom"))).toMatch(/내부 오류/);
    expect(toKoreanMessage("string error")).toMatch(/내부 오류/);
  });

  it("알 수 없는 kind는 폴백 메시지", () => {
    const out = toKoreanMessage({ kind: "weird_kind", message: "x" } as unknown);
    expect(out).toMatch(/알 수 없는/);
  });
});

describe("toToastError", () => {
  it("detail이 있으면 title + description", () => {
    const r = toToastError({ kind: "firestore", message: "INTERNAL: deadline exceeded" });
    expect(r.title).toMatch(/Firestore/);
    expect(r.description).toBe("INTERNAL: deadline exceeded");
  });

  it("detail이 비어있으면 title만", () => {
    const r = toToastError({ kind: "no_session", message: "" });
    expect(r.description).toBeUndefined();
  });

  it("긴 detail은 240자에서 잘림", () => {
    const long = "x".repeat(500);
    const r = toToastError({ kind: "internal", message: long });
    expect(r.description?.length).toBeLessThanOrEqual(240);
    expect(r.description?.endsWith("…")).toBe(true);
  });
});
