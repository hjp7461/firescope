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
- 💾 **Export** — JSON / NDJSON / CSV, post_filter 적용 시 matched/scanned 분리
- 📈 **컬렉션 통계** — 샘플(100/500/1000)에서 필드별 type 분포·NULL 비율·상위 값
- 🔗 **누락 인덱스 가이드** — Firestore 인덱스 에러 발생 시 Firebase 콘솔 링크 자동 추출
- ⭐ **저장된 쿼리** — 프로파일별 히스토리 + 핀(북마크) 분리
- 🌗 **다크모드** — 시스템/라이트/다크 토글
- ⌨️ **단축키** — `Cmd/Ctrl+Enter` 실행, `Esc` 취소, `Cmd/Ctrl+1..9` 프로파일 빠른 전환

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
pnpm tauri build  # 데스크탑 바이너리 패키징 (LTO release)
```

산출물:

- macOS: `src-tauri/target/release/bundle/macos/Firescope.app` + `dmg/Firescope_<ver>.dmg`
- Windows: `src-tauri/target/release/bundle/msi/Firescope_<ver>.msi`
- Linux: `src-tauri/target/release/bundle/{deb,appimage}/`

코드 사이닝/Notarization은 별도 인증서가 필요합니다 (현재 unsigned 빌드).

## 사용법

### 첫 실행
1. 좌측 사이드바의 `+`로 프로파일 추가 (이름, 프로젝트 ID, 인증 모드 선택).
2. 자격증명 입력 — 서비스 계정 JSON 파일을 선택하거나 ID 토큰을 붙여 넣으면 OS Vault에 저장됩니다.
3. 검증 단계에서 연결 테스트 후 저장.
4. 사이드바의 프로파일을 **더블클릭하여 활성화**.

### 쿼리
- 좌측 **Collections** 패널에서 컬렉션 클릭 → 첫 100건 표시.
- **Query Builder**에서 where/orderBy/limit/CollectionGroup/post_filter 작성.
- post_filter는 백엔드가 적용하기 어려운 패턴(정규식·contains·JSONPath)을 클라이언트 측에서 적용 — `scanned`와 `matched`가 분리 카운트됩니다.

### Export
- 결과 표시 후 `[내보내기 ▼]` → JSON / NDJSON / CSV 선택 → 저장 경로.
- post_filter 적용 시 **매칭 결과** / **후처리 이전 전체** 두 묶음으로 분기 가능.
- CSV는 모든 문서의 필드 union을 헤더로 사용 (nested 값은 JSON 문자열로).

### 단축키

| 키 | 동작 |
|---|---|
| `Cmd/Ctrl + Enter` | 현재 빌더의 쿼리 실행 |
| `Esc` | 진행 중 스트림 취소 (한글 IME 조합 중 제외) |
| `Cmd/Ctrl + 1..9` | n번째 프로파일 빠른 전환 |

## 스크린샷

> 정식 릴리스 직전에 보강 예정. 캡처 가이드:
>
> 1. `pnpm tauri dev` + `docker compose -f docker/docker-compose.yml up -d` + `docker/seed.sh`로 샘플 데이터 적재.
> 2. 캡처할 화면: ① 프로파일 사이드바 + 메인 영역, ② 쿼리 빌더(where/orderBy/post_filter), ③ Table / Tree / JSON / Log 4 뷰 전환, ④ 통계 모달, ⑤ 운영 프로파일 경고 모달 + 상시 배너, ⑥ 다크 모드.
> 3. `assets/screenshots/`에 PNG 저장 후 본 절에 `![](assets/screenshots/<name>.png)` 형태로 삽입.

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
- [x] **Phase 3** — Tree / JSON / Log 뷰 + 뷰 전환 탭
- [x] **Phase 4** — 정교한 쿼리 빌더 (전체 연산자 + 히스토리)
- [x] **Phase 5** — 클라이언트 후처리 검색 (정규식/contains/JSONPath)
- [x] **Phase 6** — Export(디스크 sink) / 카운트 / 다크모드 / 단축키
- [x] **Phase 7** — 다듬기 & 배포 (한국어 에러 매핑 / 첫 실행 온보딩 / 운영 경고 / `tauri build`)
- [x] **Phase 8** — 워크플로 가속 (인덱스 자동 가이드 / 저장된 쿼리 / 프로파일 그룹)
- [x] **Phase 9** — 컬렉션 통계 (필드별 type 분포 / NULL 비율 / 상위 값)

## 보안 약속

- 자격증명(서비스 계정 JSON, ID 토큰) 본문은 OS 자격증명 저장소에만 보관, 평문 파일 미사용
- 자격증명 본문은 IPC 응답/로그/에러 메시지에 절대 노출되지 않음
- 프로파일 메타데이터 export 시 자격증명 정보는 제외
- 운영 프로젝트 프로파일은 활성화 전 사용자 확인 + UI 경고 배너 상시 표시

## 라이선스

(미정)
