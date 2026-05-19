// 쿼리 빌더 드래프트 → QueryDsl 변환 (순수, UI/zustand 무관 — 원칙 7).
//
// UI는 타입 없는 문자열을 모은다. 여기서 사용자가 고른 값 타입에 따라
// 태그된 `FirestoreValue` 유니온으로 변환하고, 멤버십 연산자는 값 목록을
// 배열로 만든다. 파싱 실패는 throw하지 않고 Result로 돌려준다(빌더가
// 인라인 경고를 띄울 수 있도록).

import type {
  CompareOp,
  FirestoreValue,
  OrderBy,
  QueryDsl,
} from "@/types";

/** 빌더가 입력 위젯을 고르는 데 쓰는 스칼라 값 타입. */
export type DraftValueType =
  | "string"
  | "int"
  | "double"
  | "bool"
  | "null"
  | "timestamp"
  | "reference";

export type DraftWhere = {
  field: string;
  op: CompareOp;
  valueType: DraftValueType;
  /** 사용자 원본 입력. 멤버십 연산자면 구분자로 분리. */
  raw: string;
};

export type QueryDraft = {
  targetKind: "collection" | "collection_group";
  /** collection이면 경로, collection_group이면 그룹 id. */
  target: string;
  wheres: DraftWhere[];
  orderBys: OrderBy[];
  limit: number;
};

export type BuildResult =
  | { ok: true; dsl: QueryDsl }
  | { ok: false; error: string };

const ARRAY_OPS: ReadonlySet<CompareOp> = new Set<CompareOp>([
  "in",
  "not_in",
  "array_contains_any",
]);

export function isArrayOp(op: CompareOp): boolean {
  return ARRAY_OPS.has(op);
}

/** 단일 스칼라 문자열 → FirestoreValue. 실패 시 문자열 에러 메시지. */
function scalar(
  type: DraftValueType,
  raw: string,
): { ok: true; value: FirestoreValue } | { ok: false; error: string } {
  const t = raw.trim();
  switch (type) {
    case "null":
      return { ok: true, value: { type: "null" } };
    case "bool": {
      if (t === "true") return { ok: true, value: { type: "bool", value: true } };
      if (t === "false")
        return { ok: true, value: { type: "bool", value: false } };
      return { ok: false, error: `bool 값은 true/false 여야 합니다: "${t}"` };
    }
    case "int": {
      if (!/^-?\d+$/.test(t))
        return { ok: false, error: `정수가 아닙니다: "${t}"` };
      return { ok: true, value: { type: "int", value: t } };
    }
    case "double": {
      const n = Number(t);
      if (t === "" || Number.isNaN(n))
        return { ok: false, error: `숫자가 아닙니다: "${t}"` };
      return { ok: true, value: { type: "double", value: n } };
    }
    case "string":
      return { ok: true, value: { type: "string", value: raw } };
    case "timestamp":
      return { ok: true, value: { type: "timestamp", value: t } };
    case "reference":
      return { ok: true, value: { type: "reference", value: t } };
  }
}

/**
 * 멤버십 연산자(`in`/`not_in`/`array_contains_any`) 값 파싱 정책.
 *
 * TODO(사용자 결정): 5~10줄. 아래 정책 중 무엇을 적용할지 구현해 주세요.
 * 이 함수가 빌더에서 사용자가 한 칸에 친 텍스트를 어떻게 여러 값으로
 * 쪼개는지를 결정합니다 — 기능 동작을 좌우하는 핵심 선택입니다.
 *
 * 고려할 정책:
 *  (a) 쉼표 분리: "a, b, c" → ["a","b","c"] (간단·직관, 값에 쉼표 불가)
 *  (b) 줄바꿈 분리: 한 줄에 한 값 (쉼표 포함 값 허용, UI는 textarea)
 *  (c) JSON 배열: '["a","b"]' (가장 엄밀, 사용자 부담 큼)
 *
 * 제약/주의:
 *  - 빈 토큰은 버리고, 0개면 에러를 돌려주세요(백엔드 arity 규칙: 1..=30).
 *  - 각 토큰은 `scalar(type, token)`으로 변환하고 실패를 전파하세요.
 *  - 반환: 성공 `{ ok: true, values: FirestoreValue[] }`,
 *          실패 `{ ok: false, error: string }`.
 *
 * @param type 사용자가 고른 원소 값 타입
 * @param raw  멤버십 연산자 입력 칸의 원본 문자열
 */
function parseMembership(
  type: DraftValueType,
  raw: string,
): { ok: true; values: FirestoreValue[] } | { ok: false; error: string } {
  // 정책: 쉼표 분리 → trim → 빈 토큰 제거 → 토큰별 scalar 변환.
  const tokens = raw
    .split(",")
    .map((t) => t.trim())
    .filter((t) => t !== "");
  if (tokens.length === 0) {
    return { ok: false, error: "값을 쉼표로 1개 이상 입력하세요" };
  }
  const values: FirestoreValue[] = [];
  for (const tok of tokens) {
    const s = scalar(type, tok);
    if (!s.ok) return { ok: false, error: s.error };
    values.push(s.value);
  }
  return { ok: true, values };
}

export function buildDsl(draft: QueryDraft): BuildResult {
  const target = draft.target.trim();
  if (!target) {
    return { ok: false, error: "대상 컬렉션/그룹을 입력하세요" };
  }

  const where = [];
  for (const w of draft.wheres) {
    const field = w.field.trim();
    if (!field) return { ok: false, error: "where 필드명이 비어 있습니다" };

    if (isArrayOp(w.op)) {
      const parsed = parseMembership(w.valueType, w.raw);
      if (!parsed.ok) return { ok: false, error: parsed.error };
      where.push({ field, op: w.op, value: parsed.values });
    } else {
      const s = scalar(w.valueType, w.raw);
      if (!s.ok) return { ok: false, error: s.error };
      where.push({ field, op: w.op, value: s.value });
    }
  }

  const orderBys = draft.orderBys.filter((o) => o.field.trim() !== "");

  const dsl: QueryDsl = {
    target:
      draft.targetKind === "collection"
        ? { kind: "collection", path: target }
        : { kind: "collection_group", id: target },
  };
  if (where.length > 0) dsl.where = where;
  if (orderBys.length > 0) dsl.order_by = orderBys;
  if (draft.limit > 0) dsl.limit = draft.limit;

  return { ok: true, dsl };
}
