// AppError → 사용자용 한국어 메시지 매핑 (Phase 7-A1).
//
// 원칙 3·5에 따라 백엔드 영문 message는 "개발자용 detail"이고, 사용자에게는
// 일반화된 한국어 안내가 보여야 한다. 자격증명 본문은 백엔드가 이미 마스킹하지만
// 사용자 입장에서 detail 자체가 도움이 안 되는 경우(특히 internal/io)가 많다.

import { asAppError, type AppError } from "@/types";

function kindToMessage(e: AppError): string {
  switch (e.kind) {
    case "auth":
      return "인증에 실패했습니다. 자격증명을 확인하세요.";
    case "firestore":
      return "Firestore 요청이 실패했습니다.";
    case "invalid_query":
      return "쿼리가 유효하지 않습니다.";
    case "io":
      return "파일 입출력 중 오류가 발생했습니다.";
    case "internal":
      return "내부 오류가 발생했습니다.";
    case "no_session":
      return "활성 세션이 없습니다. 먼저 프로파일을 활성화하세요.";
    case "profile_not_found":
      return "프로파일을 찾을 수 없습니다.";
    case "credential_not_found":
      return "자격증명이 등록되어 있지 않습니다.";
    case "credential_invalid":
      return "자격증명 형식이 올바르지 않습니다.";
    case "confirmation_required":
      return "운영 환경 활성화는 확인이 필요합니다.";
    case "vault_error":
      return "OS 자격증명 저장소에 접근하지 못했습니다.";
    case "duplicate_profile":
      return "같은 이름의 프로파일이 이미 있습니다.";
    case "session_not_found":
      return "이 탭의 세션이 만료되었습니다. 프로파일을 다시 활성화하세요.";
    default: {
      // TypeScript exhaustiveness guard — should never reach here at runtime.
      const _exhaustive: never = e;
      void _exhaustive;
      return "알 수 없는 오류가 발생했습니다.";
    }
  }
}

/**
 * 표준 한국어 메시지만 반환. 백엔드 detail은 description으로 보조 표시할 때만 쓴다.
 *
 * AppError가 아닌 임의의 throw 값도 안전하게 처리 (asAppError로 정규화).
 */
export function toKoreanMessage(err: unknown): string {
  const e = asAppError(err);
  return kindToMessage(e);
}
