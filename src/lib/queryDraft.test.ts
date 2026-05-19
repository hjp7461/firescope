import { describe, expect, it } from "vitest";
import {
  buildDsl,
  EMPTY_POST_FILTER,
  isArrayOp,
  type QueryDraft,
} from "./queryDraft";

function draft(over: Partial<QueryDraft> = {}): QueryDraft {
  return {
    targetKind: "collection",
    target: "users",
    wheres: [],
    orderBys: [],
    limit: 100,
    postFilter: { ...EMPTY_POST_FILTER },
    ...over,
  };
}

describe("buildDsl", () => {
  it("collection 타깃을 만든다", () => {
    const r = buildDsl(draft());
    expect(r.ok && r.dsl.target).toEqual({ kind: "collection", path: "users" });
    expect(r.ok && r.dsl.limit).toBe(100);
  });

  it("collection_group 타깃을 만든다", () => {
    const r = buildDsl(draft({ targetKind: "collection_group", target: "posts" }));
    expect(r.ok && r.dsl.target).toEqual({ kind: "collection_group", id: "posts" });
  });

  it("빈 타깃은 거부", () => {
    const r = buildDsl(draft({ target: "  " }));
    expect(r.ok).toBe(false);
  });

  it("스칼라 값 타입을 태그 유니온으로 변환", () => {
    const r = buildDsl(
      draft({
        wheres: [
          { field: "active", op: "==", valueType: "bool", raw: "true" },
          { field: "age", op: ">=", valueType: "int", raw: "18" },
          { field: "score", op: "<", valueType: "double", raw: "9.5" },
          { field: "deleted", op: "==", valueType: "null", raw: "" },
          { field: "name", op: "==", valueType: "string", raw: "Ann" },
        ],
      }),
    );
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.dsl.where).toEqual([
      { field: "active", op: "==", value: { type: "bool", value: true } },
      { field: "age", op: ">=", value: { type: "int", value: "18" } },
      { field: "score", op: "<", value: { type: "double", value: 9.5 } },
      { field: "deleted", op: "==", value: { type: "null" } },
      { field: "name", op: "==", value: { type: "string", value: "Ann" } },
    ]);
  });

  it("잘못된 정수/불리언은 에러", () => {
    expect(
      buildDsl(draft({ wheres: [{ field: "a", op: "==", valueType: "int", raw: "1.5" }] })).ok,
    ).toBe(false);
    expect(
      buildDsl(draft({ wheres: [{ field: "a", op: "==", valueType: "bool", raw: "yes" }] })).ok,
    ).toBe(false);
  });

  it("빈 필드명은 에러", () => {
    const r = buildDsl(
      draft({ wheres: [{ field: "  ", op: "==", valueType: "string", raw: "x" }] }),
    );
    expect(r.ok).toBe(false);
  });

  it("멤버십 연산자는 쉼표 분리로 배열을 만든다", () => {
    const r = buildDsl(
      draft({
        wheres: [
          { field: "role", op: "in", valueType: "string", raw: "admin, editor , viewer" },
        ],
      }),
    );
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.dsl.where?.[0].value).toEqual([
      { type: "string", value: "admin" },
      { type: "string", value: "editor" },
      { type: "string", value: "viewer" },
    ]);
  });

  it("멤버십 빈 값은 에러", () => {
    const r = buildDsl(
      draft({ wheres: [{ field: "role", op: "in", valueType: "string", raw: "  ,  " }] }),
    );
    expect(r.ok).toBe(false);
  });

  it("멤버십 토큰 중 하나가 타입 불일치면 에러", () => {
    const r = buildDsl(
      draft({ wheres: [{ field: "n", op: "not_in", valueType: "int", raw: "1, two, 3" }] }),
    );
    expect(r.ok).toBe(false);
  });

  it("빈 order_by 행은 제외, limit 0이면 생략", () => {
    const r = buildDsl(
      draft({
        limit: 0,
        orderBys: [
          { field: "", direction: "asc" },
          { field: "created_at", direction: "desc" },
        ],
      }),
    );
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.dsl.order_by).toEqual([{ field: "created_at", direction: "desc" }]);
    expect(r.dsl.limit).toBeUndefined();
    expect(r.dsl.where).toBeUndefined();
  });

  it("후처리 미설정이면 post_filter 생략", () => {
    const r = buildDsl(draft());
    expect(r.ok && r.dsl.post_filter).toBeUndefined();
  });

  it("regex 후처리: 필드 쉼표 분리 + case_insensitive", () => {
    const r = buildDsl(
      draft({
        postFilter: {
          kind: "regex",
          fields: "name, profile.city",
          pattern: "iphone\\s?1[35]",
          caseInsensitive: true,
          jsonpath: "",
        },
      }),
    );
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.dsl.post_filter).toEqual({
      regex: {
        fields: ["name", "profile.city"],
        pattern: "iphone\\s?1[35]",
        case_insensitive: true,
      },
    });
  });

  it("contains 후처리", () => {
    const r = buildDsl(
      draft({
        postFilter: {
          kind: "contains",
          fields: "desc",
          pattern: "sale",
          caseInsensitive: false,
          jsonpath: "",
        },
      }),
    );
    expect(r.ok && r.dsl.post_filter).toEqual({
      contains: { fields: ["desc"], text: "sale", case_insensitive: false },
    });
  });

  it("jsonpath만 단독 설정 가능", () => {
    const r = buildDsl(
      draft({
        postFilter: {
          kind: "regex",
          fields: "",
          pattern: "",
          caseInsensitive: false,
          jsonpath: "$.tags[?@ == 'urgent']",
        },
      }),
    );
    expect(r.ok && r.dsl.post_filter).toEqual({
      jsonpath: "$.tags[?@ == 'urgent']",
    });
  });

  it("패턴은 있는데 필드가 비면 에러", () => {
    const r = buildDsl(
      draft({
        postFilter: {
          kind: "regex",
          fields: "  ",
          pattern: "x",
          caseInsensitive: false,
          jsonpath: "",
        },
      }),
    );
    expect(r.ok).toBe(false);
  });

  it("regex + jsonpath 동시 (AND)", () => {
    const r = buildDsl(
      draft({
        postFilter: {
          kind: "regex",
          fields: "name",
          pattern: "iPhone",
          caseInsensitive: false,
          jsonpath: "$.tags[?@ == 'urgent']",
        },
      }),
    );
    expect(r.ok && r.dsl.post_filter).toEqual({
      regex: { fields: ["name"], pattern: "iPhone", case_insensitive: false },
      jsonpath: "$.tags[?@ == 'urgent']",
    });
  });

  it("isArrayOp 판별", () => {
    expect(isArrayOp("in")).toBe(true);
    expect(isArrayOp("array_contains_any")).toBe(true);
    expect(isArrayOp("==")).toBe(false);
    expect(isArrayOp("array_contains")).toBe(false);
  });
});
