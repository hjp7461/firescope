# Firescope

> **A read-only window into your Firestore.**

Firebase Studio의 Firestore 검색 한계를 보완하는 **읽기 전용 Firestore 데스크탑 클라이언트**.
정교한 쿼리 UI와 **다중 프로젝트 프로파일**로 운영/스테이징/에뮬레이터를 한 곳에서 안전하게 들여다봅니다.
UX는 [Firefoo](https://www.firefoo.com/)를 참고했습니다.

> ⚠️ **이 저장소는 Claude Code 스타트킷입니다.**
> 실제 코드는 아직 작성되지 않았으며, Claude Code에서 `/kickoff` 명령으로 시작합니다.

## 핵심 특징

- 🔒 **읽기 전용** — 쓰기/수정/삭제 API 미구현
- 🚫 **API 키 미사용** — 서비스 계정 또는 ID 토큰 인증
- 👥 **다중 프로파일** — 여러 Firebase 프로젝트 등록 + 전환
- 🔐 **OS Vault 통합** — 자격증명을 macOS Keychain / Windows Credential Manager / Linux Secret Service에 저장
- 🐳 **에뮬레이터 우선 개발** — Docker 로컬 에뮬레이터
- 📊 **4가지 결과 뷰** — Table / Tree / JSON / Log
- 🔍 **Firefoo 스타일 쿼리 빌더** — where 조합, orderBy, 커서, 정규식 후처리

## 기술 스택

- **Backend**: Rust 1.80+ / Tauri 2.11 / firestore 0.44 / gcp_auth / keyring / secrecy / tokio
- **Frontend**: React 18 + TypeScript 5 + Vite 5 / TanStack Table / Zustand / Tailwind + shadcn/ui
- **Infra**: Docker Compose (Firestore + Auth Emulator)

자세한 내용은 [`CLAUDE.md`](./CLAUDE.md) 및 [`docs/`](./docs) 참조.

### 주요 문서
- [01-setup.md](./docs/01-setup.md) — 개발 환경 셋업
- [02-architecture.md](./docs/02-architecture.md) — 레이어/IPC/인증 구조
- [03-ipc-contract.md](./docs/03-ipc-contract.md) — Tauri 명령 명세
- [04-query-dsl.md](./docs/04-query-dsl.md) — 쿼리 DSL 스펙
- [05-emulator.md](./docs/05-emulator.md) — Docker 에뮬레이터 운영
- [06-roadmap.md](./docs/06-roadmap.md) — Phase별 작업 체크리스트
- **[07-profiles.md](./docs/07-profiles.md) — 다중 프로파일 / 자격증명 관리**

## Claude Code로 시작하기

```bash
cd firescope
claude
```
세션에서:
```
/kickoff
```

### 주요 슬래시 명령

| 명령 | 용도 |
|------|------|
| `/kickoff` | 현재 Phase 작업 시작 |
| `/add-command <이름>` | 새 Tauri IPC 명령 추가 워크플로 |
| `/add-profile-mode <모드>` | 새 인증 모드(예: oauth_user) 추가 워크플로 |
| `/check-readonly` | 읽기 전용 원칙 자가 검증 |
| `/check-credential-leak` | 자격증명 누출 자가 검증 |
| `/phase-complete` | 현재 Phase 마무리 + 다음 단계 안내 |

## 수동 셋업

```bash
pnpm install
pnpm emulator:up
pnpm tauri dev
```

## 디렉토리 구조

| 경로 | 설명 |
|------|------|
| `CLAUDE.md` | Claude Code 진입점 |
| `docs/` | 셋업/아키텍처/IPC/DSL/에뮬레이터/로드맵/**프로파일** 문서 |
| `.claude/` | Claude Code 설정과 슬래시 명령 |
| `docker/` | Firebase Emulator Docker 설정 |
| `src-tauri/` | Rust 백엔드 (Phase 0~1에서 생성) |
| `src/` | React 프론트엔드 (Phase 0~1에서 생성) |

## 로드맵 요약

0. 스캐폴딩 → **1. 프로파일 관리 기반** → 2. 기본 조회 + Table → 3. Tree/JSON/Log → 4. 정교한 쿼리 빌더 → 5. 후처리 검색 → 6. Export/편의 → 7. 다듬기/배포

상세는 [`docs/06-roadmap.md`](./docs/06-roadmap.md).

## 보안 약속

- 자격증명(서비스 계정 JSON, ID 토큰) 본문은 OS 자격증명 저장소에만 보관, 평문 파일 미사용
- 자격증명 본문은 IPC 응답/로그/에러 메시지에 절대 노출되지 않음
- 프로파일 메타데이터 export 시 자격증명 정보는 제외
- 운영 프로젝트 프로파일은 활성화 전 사용자 확인 + UI 경고 배너 상시 표시

## 라이선스

(미정)
