// Phase 9 컬렉션 통계 (`docs/03-ipc-contract.md` §5 compute_stats v0.7).
// Rust `query::stats`와 동기화.

import type { ExportSource } from "./query";

export type StatsReport = {
  /** 통계에 들어간 문서 수. */
  sample_size: number;
  /** sink의 어떤 부분이 사용되었는지. */
  source: ExportSource;
  /** top-level 필드, key 알파벳순. */
  fields: FieldStat[];
};

export type FieldStat = {
  key: string;
  /** 필드가 존재한 문서 수 (null도 present로 친다). */
  present: number;
  /** 필드 자체가 없었던 문서 수. */
  missing: number;
  /** FirestoreValue { type: "null" }인 문서 수. */
  null_count: number;
  /** count 내림차순, 동률 type 알파벳순. */
  types: TypeBucket[];
  /** count 내림차순, 동률 value 알파벳순. 상위 N개. */
  samples: SampleValue[];
};

export type TypeBucket = {
  type: string;
  count: number;
};

export type SampleValue = {
  value: string;
  count: number;
};

/** ComputeStatsParams: top_samples는 백엔드에서 0~50으로 클램프된다. */
export type ComputeStatsParams = {
  stream_id: string;
  source?: ExportSource;
  top_samples?: number;
};
