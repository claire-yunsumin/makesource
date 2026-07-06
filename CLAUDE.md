# CLAUDE.md — LocalBrush

macOS 로컬 AI 브랜드 그래픽 생성기. Tauri 2 + React/TS 프론트, Rust 백엔드, ComfyUI(추론)·kohya(학습) 서브프로세스.

## 문서 (구현 전 반드시 해당 섹션 확인)

- `docs/02_PRD.md` — 기능 요구사항(F-x)과 우선순위
- `docs/03_기술설계서_TAD.md` — 폴더 구조, DB/JSON 스키마, Tauri command 계약, ComfyUI 연동 스펙 ← **구현의 단일 진실**
- `docs/04_UI_디자인_스펙.md` — 화면 구조, 디자인 토큰, 컴포넌트, 문구 톤
- `docs/05_구현_백로그.md` — 태스크와 완료 기준(AC)

## 명령어

```bash
pnpm install          # 의존성
pnpm tauri dev        # 개발 실행
pnpm tauri build      # .dmg 빌드
pnpm test             # Vitest
pnpm lint             # ESLint + Prettier check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

## 절대 규칙

1. **로컬 전용**: 이미지·프롬프트를 외부로 전송하는 코드 금지. 허용된 네트워크는 Hugging Face 모델 다운로드, GitHub 업데이트 확인뿐. 크래시 리포터·외부 텔레메트리 추가 금지
2. **유료 의존성 금지**: 유료 API/SaaS/폰트 사용 불가. 새 의존성은 MIT/Apache/BSD 우선, GPL은 ComfyUI(별도 프로세스) 외 추가 금지
3. **IPC 계약 준수**: Tauri command 시그니처는 TAD §5가 기준. 변경 시 TAD 문서를 같은 PR에서 갱신하고 `src/lib/tauri.ts` 타입 동기화
4. **long-running 작업 패턴**: command는 jobId만 반환, 진행은 Tauri event(`gen://`, `train://`, `bootstrap://` 등)로 push. 블로킹 command 금지
5. **에러 규약**: Rust는 AppError(code/message/detail)로 통일, 사용자 메시지는 한국어(04 §6 톤). unwrap/expect는 테스트 코드에서만

## 세션 작업 루프 (태스크 하나를 끝까지)

1. **읽기**: docs/05에서 태스크·AC 확인 → 해당 TAD·04 섹션 정독 → 관련 기존 코드(비슷한 기능의 command/화면/테스트) 훑기. 기존 패턴을 새로 만들지 말고 재사용
2. **분기**: 항상 최신 main에서. 선행 PR 위에 스택하지 말 것 (squash merge라 rebase 지옥) — 의존되면 먼저 머지
3. **구현**: 순수 로직 테스트 먼저 → 구현 → 검증. 시간이 걸리는 cargo 검증은 백그라운드로 돌리고 그동안 프론트 작업
4. **같은 PR에서 동기화**: command 계약 변경 시 TAD §5 + `src/lib/tauri.ts`, 완료 태스크는 docs/05 체크, 되돌리기 비싼 결정은 docs/06에 D-xxx, 새 모델·의존성은 라이선스(MIT/Apache/BSD)와 다운로드 출처(HF만) 확인
5. **PR 루프**: `gh pr create`(본문: 변경 내용 + AC 검증 결과 + 실기기 수동 확인 항목) → `gh pr checks --watch` → 그린이면 `gh pr merge --squash --delete-branch` → main pull 후 다음 태스크
6. **막히면**: 제품 축을 바꾸는 결정(프리셋 구성, 계약 형태 등)은 임의로 정하지 말고 사용자에게 선택지를 제시

검증 한 번에:
```bash
pnpm test && pnpm exec tsc --noEmit && pnpm lint
cargo fmt --check --manifest-path src-tauri/Cargo.toml && \
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings && \
cargo test --manifest-path src-tauri/Cargo.toml
```

## 브랜치 전략 (트렁크 기반)

- `main`: 항상 빌드 가능. 직접 push 금지 — 반드시 PR + CI 통과 후 **squash merge**
- 작업 브랜치: `feat|fix|docs|chore/T{태스크ID}-{짧은설명}` (예: `feat/T1.3-workflow-slot`). 백로그 태스크 1개 = 브랜치 1개 = PR 1개
- PR 설명에 해당 태스크 AC 검증 결과 기록 (자동 테스트 + 수동 확인 항목)
- 릴리스: 릴리스 브랜치 없음. `vX.Y.Z` 태그 push → CI가 .dmg 빌드 후 GitHub Release 업로드
- 병렬 세션은 `git worktree`로 폴더 분리. 단, `src/lib/tauri.ts`나 TAD §5 command 계약을 건드리는 태스크는 병렬 금지(순차 진행)
- 브랜치는 머지 후 즉시 삭제하고 main에서 새로 분기 (장수 브랜치 금지)

## 코드 컨벤션

- 프론트: 함수형 컴포넌트, 화면 상태는 화면 폴더 내 로컬 store, 전역은 `stores/`의 Zustand 최소 사용. 스타일은 Tailwind + 04 §2 토큰만 (임의 hex 금지)
- Rust: 모듈 경계는 TAD §2 구조 준수 (`commands/`는 얇게 — 로직은 `engine/ training/ prompt/ db/`에)
- Python 스크립트(`python/`): stdin/argv 입력 → stdout JSON 한 줄 출력, 로그는 stderr. 앱과의 계약을 깨지 말 것
- 커밋: Conventional Commits (`feat: T1.3 워크플로 슬롯 치환기`)— 백로그 태스크 ID 포함

## 테스트

- 순수 로직(프롬프트 조립, 슬롯 치환, stdout 파서)은 유닛 테스트 필수 — 구현 전 AC 기반 테스트 먼저 작성
- GPU가 필요한 실제 생성은 자동화하지 않음: `docs/05` AC를 수동 검증하고 결과를 PR 설명에 기록

## 주의: 자주 하는 실수

- ComfyUI 워크플로 JSON의 노드 ID를 하드코딩하지 말 것 — `_slot` 키 기반 치환 사용 (TAD §6)
- 모델/대용량 파일을 리포에 커밋 금지 (앱 데이터 폴더에서 다운로드·관리)
- 히스토리 이미지 경로는 앱 데이터 루트 기준 상대 경로로 저장 (사용자가 저장 위치를 옮길 수 있음)
- 같은 폴더에 대소문자만 다른 파일명 금지 (`EssenceWizard.tsx` + `essenceWizard.ts` → macOS/tsc 충돌). 컴포넌트는 PascalCase.tsx, 로직은 다른 이름의 camelCase.ts
- Rust `const &str`(include_str! 템플릿 등)는 사용처마다 인라인됨 — 동일성 비교는 `std::ptr::eq`가 아니라 내용 비교(`==`)로
- 이벤트 페이로드(gen://done 등) 필드 추가도 IPC 계약 변경 — `src/lib/tauri.ts` 타입을 같은 PR에서 갱신
