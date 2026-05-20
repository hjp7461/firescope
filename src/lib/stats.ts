// Phase 9 통계 표시 유틸 — 타입 색상 팔레트 + 퍼센트 포맷.
// 순수 함수만 두고 React 의존성은 만들지 않는다 (vitest로 단위 검증).

export type TypeColor = {
  /** stacked bar 채움 색. */
  bg: string;
  /** 범례 텍스트/배지 색. */
  text: string;
};

const FALLBACK: TypeColor = { bg: "bg-slate-400", text: "text-slate-600" };

/** Rust `FirestoreValue` 변형 이름 → 대비 가능한 Tailwind 색상. */
const PALETTE: Record<string, TypeColor> = {
  null: { bg: "bg-zinc-400", text: "text-zinc-600" },
  bool: { bg: "bg-amber-400", text: "text-amber-700" },
  int: { bg: "bg-emerald-500", text: "text-emerald-700" },
  double: { bg: "bg-green-500", text: "text-green-700" },
  string: { bg: "bg-blue-500", text: "text-blue-700" },
  bytes: { bg: "bg-stone-400", text: "text-stone-600" },
  timestamp: { bg: "bg-indigo-500", text: "text-indigo-700" },
  reference: { bg: "bg-cyan-500", text: "text-cyan-700" },
  geo: { bg: "bg-pink-500", text: "text-pink-700" },
  array: { bg: "bg-purple-500", text: "text-purple-700" },
  map: { bg: "bg-violet-500", text: "text-violet-700" },
};

export function typeColor(t: string): TypeColor {
  return PALETTE[t] ?? FALLBACK;
}

/**
 * `num/denom`을 사람이 읽기 좋은 퍼센트 문자열로.
 * - `denom === 0` → `"0%"`
 * - `num === 0` → `"0%"` (소수점 표기 없이)
 * - 10% 미만은 소수점 1자리, 이상은 정수
 * - 0보다 큰데 0.1% 미만은 `"<0.1%"`
 */
export function percent(num: number, denom: number): string {
  if (denom <= 0 || num <= 0) return "0%";
  const pct = (num / denom) * 100;
  if (pct < 0.1) return "<0.1%";
  return `${pct >= 10 ? Math.round(pct) : pct.toFixed(1)}%`;
}
