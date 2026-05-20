import { describe, expect, it } from "vitest";
import { toKoreanMessage } from "./errorMessages";

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

  it("session_not_found maps to Korean message", () => {
    expect(toKoreanMessage({ kind: "session_not_found", session_id: "x", message: "no" }))
      .toContain("세션이 만료");
  });

  it("session_limit_reached interpolates counts", () => {
    expect(
      toKoreanMessage({ kind: "session_limit_reached", active: 11, max: 10, message: "soft cap" } as any),
    ).toContain("11");
  });
});
