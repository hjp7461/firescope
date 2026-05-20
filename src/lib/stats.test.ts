import { describe, expect, it } from "vitest";
import { percent, typeColor } from "./stats";

describe("typeColor", () => {
  it("매핑된 타입은 고유 색을 반환", () => {
    const colors = ["string", "int", "null", "bool", "timestamp"].map(
      (t) => typeColor(t).bg,
    );
    // 모두 서로 다른 색이어야 한다 (시각 구분 보장)
    expect(new Set(colors).size).toBe(colors.length);
  });

  it("모르는 타입은 fallback 색", () => {
    const c = typeColor("unknown_type_xyz");
    expect(c.bg).toBe("bg-slate-400");
  });
});

describe("percent", () => {
  it("0/0 = 0%", () => {
    expect(percent(0, 0)).toBe("0%");
  });

  it("0/N = 0%", () => {
    expect(percent(0, 100)).toBe("0%");
  });

  it("정확한 % 정수 라운드", () => {
    expect(percent(50, 100)).toBe("50%");
    expect(percent(75, 100)).toBe("75%");
  });

  it("10% 미만은 소수점 1자리", () => {
    expect(percent(5, 100)).toBe("5.0%");
    expect(percent(33, 1000)).toBe("3.3%");
  });

  it("0보다 크지만 0.1% 미만은 <0.1%", () => {
    expect(percent(1, 100_000)).toBe("<0.1%");
  });

  it("음수 분모는 0% (안전망)", () => {
    expect(percent(10, -1)).toBe("0%");
  });
});
