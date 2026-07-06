# LocalBrush

[![CI](https://github.com/claire-yunsumin/makesource/actions/workflows/ci.yml/badge.svg)](https://github.com/claire-yunsumin/makesource/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**내 브랜드 이미지를 내 맥에서 학습시키고, 키워드만 입력하면 브랜드 톤의 그래픽을 생성·다운로드하는 설치형 macOS 앱.**

업로드·학습·생성·저장의 전 과정이 기기 안에서 종결됩니다. 이미지와 프롬프트는 외부로 전송되지 않습니다 (허용된 네트워크는 Hugging Face 모델 다운로드와 GitHub 업데이트 확인뿐).

## 핵심 기능

- **스타일 에센스 추출** — 참조 이미지 몇 장에서 로컬 VLM이 스타일 프롬프트를 자동 추출, IP-Adapter와 결합해 학습 없이 브랜드 톤 재현
- **로컬 스타일 학습** — 사용자 이미지 20장 이상으로 LoRA를 기기 내에서 학습 (kohya)
- **프리셋 UX** — "이미지 타입 선택 + 개체 키워드 입력" 2단계만으로 생성. 프롬프트 엔지니어링은 버전 관리되는 프리셋이 뒷단에서 처리
- **완전 로컬** — 크래시 리포터·텔레메트리 없음

## 기술 스택

| 레이어 | 기술 |
|---|---|
| 앱 셸 | Tauri 2 (macOS) |
| 프론트엔드 | React 18 + TypeScript + Vite + Tailwind CSS + Zustand |
| 백엔드 | Rust (sqlx + SQLite) |
| 추론 | ComfyUI — 저장소에 포함하지 않고 별도 프로세스로 기동·HTTP/WS 통신 |
| 학습 | kohya (서브프로세스) |

## 설치

1. [Releases](https://github.com/claire-yunsumin/makesource/releases)에서 최신 `LocalBrush_x.y.z_aarch64.dmg`(Apple Silicon)를 내려받습니다.
2. dmg를 열고 LocalBrush를 Applications 폴더로 드래그합니다.
3. 처음 실행하면 macOS가 **"확인되지 않은 개발자"** 경고를 띄웁니다 — 이 앱은 애플 공증(유료, 연 $99)을 받지 않은 무서명 배포이기 때문입니다. Finder에서 앱을 **우클릭(또는 control-클릭) → 열기**를 누르고, 뜨는 대화상자에서 다시 **열기**를 누르면 실행됩니다. (처음 한 번만 필요)
4. 첫 실행 시 최초 설치 화면이 뜹니다. 표준(~10GB)/라이트(~4GB) 중 하나를 골라 모델을 내려받으면(인터넷 필요) 바로 쓸 수 있습니다.

Apple Silicon(M1 이상) macOS 전용입니다. Intel Mac에서는 코드 빌드는 되지만 이미지 생성(ComfyUI/MPS)이 동작하지 않습니다.

Homebrew로도 설치할 수 있습니다:

```bash
brew install --cask claire-yunsumin/localbrush/localbrush
```

([homebrew-localbrush](https://github.com/claire-yunsumin/homebrew-localbrush) tap. 릴리스 직후에는 캐스크의 버전·체크섬이 아직 최신이 아닐 수 있습니다 — 그럴 땐 위 dmg 직접 설치를 이용해주세요.)

## 개발 환경 세팅

요구 사항: macOS, Node.js 20+, pnpm, Rust(stable, `~/.cargo/bin`이 PATH에 있어야 함)

```bash
pnpm install          # 의존성 설치
pnpm tauri dev        # 개발 실행
pnpm tauri build      # .dmg 빌드
```

검증:

```bash
pnpm test             # Vitest
pnpm lint             # ESLint + Prettier check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

## 문서

| 문서 | 내용 |
|---|---|
| [docs/02_PRD.md](docs/02_PRD.md) | 기능 요구사항(F-x)과 우선순위 |
| [docs/03_기술설계서_TAD.md](docs/03_기술설계서_TAD.md) | 폴더 구조, DB/JSON 스키마, Tauri command 계약, ComfyUI 연동 스펙 — **구현의 단일 진실** |
| [docs/04_UI_디자인_스펙.md](docs/04_UI_디자인_스펙.md) | 화면 구조, 디자인 토큰, 컴포넌트, 문구 톤 |
| [docs/05_구현_백로그.md](docs/05_구현_백로그.md) | 태스크와 완료 기준(AC) |
| [docs/06_결정기록.md](docs/06_결정기록.md) | 주요 기술·운영 결정 기록과 미결정 사항 추적 |
| [docs/07_프리셋_프롬프트_연구.md](docs/07_프리셋_프롬프트_연구.md) | 프리셋 6종 프롬프트 실험 기록 |
| [docs/08_수동_QA_체크리스트.md](docs/08_수동_QA_체크리스트.md) | 배포 전 실기기(Apple Silicon) 수동 검증 체크리스트 |

기여 규칙(브랜치 전략, 커밋 컨벤션, IPC 계약 등)은 [CLAUDE.md](CLAUDE.md)를 참고하세요.

## 라이선스

[MIT](LICENSE). 이 저장소의 코드에만 적용됩니다 — ComfyUI(GPL-3.0)는 저장소에 포함되지 않으며, 사용자 기기에 별도 설치되어 독립 프로세스로 실행됩니다.
