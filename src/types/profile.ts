// IPC 계약(`docs/03-ipc-contract.md`)과 동기화된 TS 타입.
// 원칙 2(Contract-First)·10(Data-First UI): 이 타입이 단일 진실 원천이며
// Zustand 스토어는 이들의 부분집합/파생만 보관한다.

export type ProfileMode = "emulator" | "service_account" | "id_token";

export type ProfileMeta = {
  id: string;
  name: string;
  description?: string;
  project_id: string;
  mode: ProfileMode;
  color?: string;
  tags?: string[];
  /** Phase 8-C: 사이드바 분류 그룹 (자유 문자열, 없으면 "그룹 없음"). */
  group?: string;
  firestore_host?: string;
  auth_host?: string;
  require_confirmation: boolean;
  read_only_warning: boolean;
  has_credential: boolean;
  created_at: string;
  last_used_at?: string;
  use_count: number;
};

export type CreateProfileParams = {
  name: string;
  description?: string;
  project_id: string;
  mode: ProfileMode;
  color?: string;
  tags?: string[];
  /** Phase 8-C: 사이드바 분류 그룹 (자유 문자열, 없으면 "그룹 없음"). */
  group?: string;
  firestore_host?: string;
  auth_host?: string;
  require_confirmation?: boolean;
  read_only_warning?: boolean;
};

export type UpdateProfileParams = {
  profile_id: string;
  name?: string;
  description?: string;
  color?: string;
  tags?: string[];
  /** Phase 8-C: 사이드바 분류 그룹 (자유 문자열, 없으면 "그룹 없음"). */
  group?: string;
  firestore_host?: string;
  auth_host?: string;
  require_confirmation?: boolean;
  read_only_warning?: boolean;
};

export type CredentialInput =
  | { kind: "service_account"; json: string }
  | { kind: "id_token"; token: string };

export type CredentialStatus = { has_credential: boolean };

export type TestResult = {
  ok: true;
  project_id: string;
  latency_ms: number;
};

export type ExportResult = { written_bytes: number; count: number };

export type ImportDetail = {
  name: string;
  status: "imported" | "skipped";
  reason?: string;
};

export type ImportResult = {
  imported: number;
  skipped: number;
  details: ImportDetail[];
};
