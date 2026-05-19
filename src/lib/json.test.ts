import { describe, expect, it } from "vitest";
import { toDisplayJson } from "./json";
import type { FirestoreDocument } from "@/types";

const doc = (id: string, data: FirestoreDocument["data"]): FirestoreDocument => ({
  path: `c/${id}`, id, parent: "c", data, create_time: null, update_time: null,
});

describe("toDisplayJson", () => {
  it("문서를 {id: {fields}} 배열로 직렬화", () => {
    const out = toDisplayJson([doc("a", { n: { type: "string", value: "x" } })]);
    expect(JSON.parse(out)).toEqual([{ a: { n: "x" } }]);
  });
  it("특수타입 엔벨로프", () => {
    const out = toDisplayJson([
      doc("d", {
        g: { type: "geo", lat: 1, lng: 2 },
        t: { type: "timestamp", value: "2026-05-20T00:00:00Z" },
        r: { type: "reference", value: "users/1" },
        b: { type: "bytes", value: "AQI=" },
        i: { type: "int", value: "42" },
      }),
    ]);
    expect(JSON.parse(out)[0].d).toEqual({
      g: { __type__: "geopoint", lat: 1, lng: 2 },
      t: "2026-05-20T00:00:00Z",
      r: "users/1",
      b: { __type__: "bytes", base64: "AQI=" },
      i: 42,
    });
  });
  it("빈 결과 → []", () => {
    expect(toDisplayJson([])).toBe("[]");
  });
});
