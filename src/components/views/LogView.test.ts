import { describe, expect, it } from "vitest";
import { formatLogLine } from "./LogView";
import type { LogEntry } from "@/types";

function entry(over: Partial<LogEntry>): LogEntry {
  return {
    ts: "2026-05-20T07:30:15.123Z",
    level: "info",
    target: "test",
    message: "hello",
    ...over,
  } as LogEntry;
}

describe("formatLogLine", () => {
  it("화면 표시 컬럼 순서를 유지: time + level + message", () => {
    expect(formatLogLine(entry({}))).toBe("07:30:15 INFO  hello");
  });

  it("level별 5자 패딩으로 정렬", () => {
    expect(formatLogLine(entry({ level: "error" }))).toBe("07:30:15 ERROR hello");
    expect(formatLogLine(entry({ level: "warn" }))).toBe("07:30:15 WARN  hello");
    expect(formatLogLine(entry({ level: "debug" }))).toBe("07:30:15 DEBUG hello");
  });

  it("ISO 타임스탬프의 시간 부분만 추출", () => {
    expect(formatLogLine(entry({ ts: "2026-12-31T23:59:59.999Z" }))).toMatch(
      /^23:59:59 /,
    );
  });
});
