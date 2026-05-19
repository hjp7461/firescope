import { describe, expect, it } from "vitest";
import { appendBounded } from "./ringBuffer";

describe("appendBounded", () => {
  it("상한 미만이면 그대로 누적", () => {
    expect(appendBounded([1, 2], 3, 5)).toEqual([1, 2, 3]);
  });
  it("상한 초과 시 앞에서 FIFO drop", () => {
    expect(appendBounded([1, 2, 3], 4, 3)).toEqual([2, 3, 4]);
  });
  it("max=1 경계", () => {
    expect(appendBounded([9], 10, 1)).toEqual([10]);
  });
});
