import { describe, expect, it } from "vitest";
import { buildTree } from "./tree";
import type { FirestoreDocument } from "@/types";

const doc = (id: string, data: FirestoreDocument["data"]): FirestoreDocument => ({
  path: `c/${id}`, id, parent: "c", data, create_time: null, update_time: null,
});

describe("buildTree", () => {
  it("문서가 최상위 노드(Type=Document)", () => {
    const t = buildTree([doc("a", { n: { type: "string", value: "x" } })]);
    expect(t).toHaveLength(1);
    expect(t[0]).toMatchObject({ id: "a", k: "a", typeLabel: "Document" });
    expect(t[0].children?.[0]).toMatchObject({
      id: "a.n", k: "n", valuePreview: "x", typeLabel: "String",
    });
  });
  it("Map/Array는 펼침 자식 + 프리뷰 카운트", () => {
    const t = buildTree([
      doc("d", {
        m: { type: "map", value: { a: { type: "int", value: "1" } } },
        arr: { type: "array", value: [{ type: "bool", value: true }] },
      }),
    ]);
    const m = t[0].children!.find((c) => c.k === "m")!;
    const arr = t[0].children!.find((c) => c.k === "arr")!;
    expect(m).toMatchObject({ typeLabel: "Map", valuePreview: "{1}" });
    expect(m.children![0]).toMatchObject({ id: "d.m.a", typeLabel: "Int" });
    expect(arr).toMatchObject({ typeLabel: "Array", valuePreview: "[1]" });
    expect(arr.children![0]).toMatchObject({ id: "d.arr.0", k: "0", typeLabel: "Boolean" });
  });
  it("geo/null/timestamp 타입 라벨", () => {
    const t = buildTree([
      doc("g", {
        loc: { type: "geo", lat: 1, lng: 2 },
        z: { type: "null" },
        ts: { type: "timestamp", value: "2026-05-20T00:00:00Z" },
      }),
    ]);
    const by = (k: string) => t[0].children!.find((c) => c.k === k)!;
    expect(by("loc")).toMatchObject({ typeLabel: "Geopoint", valuePreview: "(1, 2)" });
    expect(by("z")).toMatchObject({ typeLabel: "Null", valuePreview: "null" });
    expect(by("ts")).toMatchObject({ typeLabel: "Timestamp" });
  });
  it("빈 결과 → 빈 배열", () => {
    expect(buildTree([])).toEqual([]);
  });
});
