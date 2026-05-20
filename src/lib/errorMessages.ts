// AppError → 사용자용 한국어 메시지 매핑 (Phase 7-A1).
//
// 원칙 3·5에 따라 백엔드 영문 message는 "개발자용 detail"이고, 사용자에게는
// 일반화된 한국어 안내가 보여야 한다. 자격증명 본문은 백엔드가 이미 마스킹하지만
// 사용자 입장에서 detail 자체가 도움이 안 되는 경우(특히 internal/io)가 많다.

import { asAppError, type AppErrorKind } from "@/types";

const KIND_MESSAGES: Record<AppErrorKind, string> = {
  auth: "인증에 실패했습니다. 자격증명을 확인하세요.",
  firestore: "Firestore 요청이 실패했습니다.",
  invalid_query: "쿼리가 유효하지 않습니다.",
  io: "파일 입출력 중 오류가 발생했습니다.",
  internal: "내부 오류가 발생했습니다.",
  no_session: "활성 세션이 없습니다. 먼저 프로파일을 활성화하세요.",
  profile_not_found: "프로파일을 찾을 수 없습니다.",
  credential_not_found: "자격증명이 등록되어 있지 않습니다.",
  credential_invalid: "자격증명 형식이 올바르지 않습니다.",
  confirmation_required: "운영 환경 활성화는 확인이 필요합니다.",
  vault_error: "OS 자격증명 저장소에 접근하지 못했습니다.",
  duplicate_profile: "같은 이름의 프로파일이 이미 있습니다.",
};

/**
 * 표준 한국어 메시지만 반환. 백엔드 detail은 description으로 보조 표시할 때만 쓴다.
 *
 * AppError가 아닌 임의의 throw 값도 안전하게 처리 (asAppError로 정규화).
 */
export function toKoreanMessage(err: unknown): string {
  const e = asAppError(err);
  return KIND_MESSAGES[e.kind] ?? "알 수 없는 오류가 발생했습니다.";
}
