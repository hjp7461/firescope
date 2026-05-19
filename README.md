# Firescope

> **A read-only window into your Firestore.**

Firebase Studio의 Firestore 검색 한계를 보완하는 **읽기 전용 Firestore 데스크탑 클라이언트**.
정교한 쿼리 UI와 **다중 프로젝트 프로파일**로 운영/스테이징/에뮬레이터를 한 곳에서 안전하게 들여다봅니다.
UX는 [Firefoo](https://www.firefoo.com/)를 참고했습니다.

## 핵심 특징

- 🔒 **읽기 전용** — 쓰기/수정/삭제 API 미구현
- 🚫 **API 키 미사용** — 서비스 계정 또는 ID 토큰 인증
- 👥 **다중 프로파일** — 여러 Firebase 프로젝트 등록 + 전환
- 🔐 **OS Vault 통합** — 자격증명을 macOS Keychain / Windows Credential Manager / Linux Secret Service에 저장
- 🐳 **에뮬레이터 우선 개발** — Docker 로컬 에뮬레이터
- 📊 **4가지 결과 뷰** — Table / Tree / JSON / Log
- 🔍 **Firefoo 스타일 쿼리 빌더** — where 조합, orderBy, 커서, 정규식 후처리

## 기술 스택

- **Backend**: Rust / Tauri 2.11 / firestore 0.44 / gcp_auth / keyring / secrecy / tokio
- **Frontend**: React 19 + TypeScript 5 + Vite 7 / TanStack Table + Virtual / Zustand / Tailwind + shadcn/ui
- **Infra**: Docker Compose (Firestore + Auth Emulator)

## 개발 환경

요구 사항: Rust 1.80+, Node.js 20+, pnpm, Docker.

```bash
# 1) 로컬 Firestore/Auth 에뮬레이터 기동
docker compose -f docker/docker-compose.yml up -d

# 2) 의존성 설치
pnpm install

# 3) 데스크탑 앱 실행 (개발 모드)
pnpm tauri dev
```

개발 시 사용하는 에뮬레이터 환경변수:

```bash
FIRESTORE_EMULATOR_HOST=localhost:8080
FIREBASE_AUTH_EMULATOR_HOST=localhost:9099
GCLOUD_PROJECT=demo-firescope
```

`docker/seed.sh`로 샘플 데이터(users/posts)를 에뮬레이터에 적재할 수 있습니다.

빌드:

```bash
pnpm build        # 프론트엔드 타입체크 + 번들
pnpm tauri build  # 데스크탑 바이너리 패키징
```

## 디렉토리 구조

| 경로 | 설명 |
|------|------|
| `docker/` | Firebase Emulator Docker 설정 + 시드 스크립트 |
| `src-tauri/` | Rust 백엔드 (Tauri IPC / 인증 / 프로파일 / Firestore / 쿼리) |
| `src/` | React 프론트엔드 (UI / 스토어 / IPC 래퍼 / 타입) |

## 로드맵

- [x] **Phase 0** — 스캐폴딩 (Tauri + Vite + shadcn/ui)
- [x] **Phase 1** — 프로파일 관리 기반 (CRUD + OS Vault + 세션)
- [x] **Phase 2** — 기본 조회 + Table 뷰 (스트리밍 + 가상화 테이블)
- [ ] **Phase 3** — Tree / JSON / Log 뷰 + 뷰 전환 탭
- [ ] **Phase 4** — 정교한 쿼리 빌더 (전체 연산자 + 히스토리)
- [ ] **Phase 5** — 클라이언트 후처리 검색 (정규식/contains)
- [ ] **Phase 6** — Export & 편의 기능
- [ ] **Phase 7** — 다듬기 & 배포

## 보안 약속

- 자격증명(서비스 계정 JSON, ID 토큰) 본문은 OS 자격증명 저장소에만 보관, 평문 파일 미사용
- 자격증명 본문은 IPC 응답/로그/에러 메시지에 절대 노출되지 않음
- 프로파일 메타데이터 export 시 자격증명 정보는 제외
- 운영 프로젝트 프로파일은 활성화 전 사용자 확인 + UI 경고 배너 상시 표시

## 라이선스

(미정)
