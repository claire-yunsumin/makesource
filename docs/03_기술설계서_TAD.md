# 기술설계서 (TAD: Technical Architecture Document)
## LocalBrush — Claude Code 구현용 상세 스펙

버전: v0.1 | 대상 독자: 구현 담당(Claude Code 포함) | 상위 문서: 02_PRD.md

---

## 1. 기술 스택 확정

| 레이어 | 선택 | 비고 |
|---|---|---|
| 앱 셸 | Tauri 2 (Rust) | .dmg 빌드, 자동 업데이트 |
| 프론트엔드 | React 18 + TypeScript + Vite | |
| 상태 관리 | Zustand | 전역 최소화, 화면 로컬 상태 우선 |
| 스타일 | Tailwind CSS + 디자인 토큰(04 문서) | |
| 로컬 DB | SQLite (sqlx, Rust `db` 모듈) | Tauri command로 노출(§5). 프론트에서 SQL 직접 접근(tauri-plugin-sql) 미사용 |
| 추론 엔진 | ComfyUI (헤드리스, localhost:8188) | 서브프로세스로 기동/관리 |
| 학습 엔진 | kohya sd-scripts | 서브프로세스, stdout 파싱 |
| 캡셔닝/에센스 | Florence-2 (MIT) + WD14 Tagger | Python 스크립트 |
| Python 환경 | uv로 격리 venv 자동 구성 | 앱 데이터 폴더 내 설치 |
| 모델 | SDXL 1.0 base + IP-Adapter (기본), SD 1.5 (8GB 폴백) | HF에서 다운로드 |

## 2. 리포지토리 구조

```
localbrush/
├── CLAUDE.md                  # Claude Code 작업 지침
├── docs/                      # 00~05 기획·설계 문서
├── src/                       # React 프론트엔드
│   ├── app/                   # 라우팅, 레이아웃, 프로바이더
│   ├── screens/
│   │   ├── generate/          # 생성 화면
│   │   ├── gallery/           # 갤러리 화면
│   │   ├── styles/            # 스타일(에센스/LoRA) 관리
│   │   ├── presets/           # 프리셋 편집기
│   │   └── settings/          # 설정, 모델 관리
│   ├── components/            # 공용 컴포넌트 (04 문서 목록 기준)
│   ├── stores/                # Zustand 스토어
│   ├── lib/
│   │   ├── tauri.ts           # invoke 래퍼 (타입 안전)
│   │   └── i18n/              # ko/en
│   └── types/                 # 공용 타입 (Rust와 스키마 동기화)
├── src-tauri/                 # Rust 백엔드
│   ├── src/
│   │   ├── commands/          # Tauri command 모듈 (§5)
│   │   ├── engine/            # ComfyUI 프로세스 관리 + API 클라이언트
│   │   ├── training/          # kohya 잡 러너
│   │   ├── bootstrap/         # 최초 실행 설치기 (§7)
│   │   ├── db/                # SQLite 마이그레이션·쿼리
│   │   └── prompt/            # 프리셋 → 최종 프롬프트/워크플로 조립
│   └── resources/
│       ├── workflows/         # ComfyUI 워크플로 JSON 템플릿 (§6)
│       └── presets.default.json
├── python/                    # 앱이 배포하는 파이썬 스크립트
│   ├── caption.py             # Florence-2 + WD14 캡셔닝
│   ├── essence.py             # 참조 이미지 → 에센스 프롬프트
│   └── translate.py           # 한→영 (Argos)
└── scripts/                   # dev 편의 스크립트
```

## 3. 데이터 저장 설계

앱 데이터 루트: `~/Library/Application Support/LocalBrush/`

```
models/checkpoints/  models/loras/  models/ipadapter/  models/clip_vision/
models/argos/        models/rembg/  models/hf/ (HF 캐시 — 에센스 분석 모델)
runtime/comfyui/     runtime/venv/  runtime/kohya/
outputs/YYYY-MM/     training/{style_id}/dataset/  styles/{style_id}/ (에센스 참조 이미지)
presets.json         styles.json    app.db  logs/
```

### 3.1 SQLite 스키마 (초기 마이그레이션)

```sql
CREATE TABLE generations (
  id TEXT PRIMARY KEY,             -- uuid
  created_at INTEGER NOT NULL,     -- unix ms
  image_path TEXT NOT NULL,
  thumb_path TEXT NOT NULL,
  keyword_ko TEXT, prompt_final TEXT NOT NULL, negative TEXT,
  preset_id TEXT, preset_version INTEGER,
  style_id TEXT,                   -- essence 또는 lora 스타일
  seed INTEGER NOT NULL, steps INTEGER, cfg REAL,
  width INTEGER, height INTEGER, model TEXT,
  favorite INTEGER DEFAULT 0
);
CREATE INDEX idx_gen_created ON generations(created_at DESC);

CREATE TABLE training_jobs (
  id TEXT PRIMARY KEY, style_id TEXT NOT NULL,
  status TEXT NOT NULL,            -- queued|captioning|training|done|failed|canceled
  progress REAL DEFAULT 0, eta_seconds INTEGER,
  params_json TEXT, error TEXT,
  started_at INTEGER, finished_at INTEGER
);
```

### 3.2 presets.json 스키마

```jsonc
{
  "schemaVersion": 1,
  "presets": [{
    "id": "storybook",
    "label": { "ko": "동화같은", "en": "Storybook" },
    "version": 3,
    "history": [ /* 이전 버전 스냅샷 배열 */ ],
    "successCriteria": "파스텔톤 유지, 외곽선 없음, 배경 단순",
    "prefix": "cinematic illustration of",
    "suffix": "creating dreamlike atmosphere, soft pastel colors",
    "negative": "text, watermark, photo-realistic",
    "params": { "steps": 28, "cfg": 6.5, "width": 1024, "height": 1024 }
  }]
}
```

### 3.3 styles.json 스키마

```jsonc
{
  "styles": [{
    "id": "uuid", "name": "우리 브랜드",
    "kind": "essence" | "lora",
    "essencePrompt": "flat vector illustration, ...",   // kind=essence
    "referenceImages": ["path", ...],                    // IP-Adapter 입력
    "ipAdapterWeight": 0.6,
    "loraPath": "models/loras/brand_v1.safetensors",     // kind=lora
    "loraWeight": 0.8, "triggerWord": "brandstyle",
    "thumb": "path", "createdAt": 0
  }]
}
```

## 4. 프롬프트 조립 규칙 (prompt/ 모듈)

최종 프롬프트 = `[preset.prefix] + [스타일 트리거워드?] + [키워드(영문 변환)] + [style.essencePrompt?] + [preset.suffix]`

- 키워드 한→영: ① 도메인 용어 사전(JSON, 최우선) → ② Argos Translate → ③ 실패 시 원문 그대로 + UI 경고
- negative = preset.negative + 전역 안전 네거티브(설정에서 관리)
- 시드: 미지정 시 랜덤, "변형" 버튼은 시드 고정 + 서브시드 변경

## 5. Tauri Commands (IPC 계약)

모든 command는 `Result<T, AppError>` 반환. AppError = `{ code, message, detail? }`.
long-running 작업은 command로 시작하고 진행 상황은 **Tauri event**로 push.

| Command | 입력 | 출력 | 이벤트 |
|---|---|---|---|
| `bootstrap_status` | – | `{ step, progress, ready, suggestedProfile }` | `bootstrap://progress` |
| `bootstrap_run` | `{ modelProfile }` | jobId | 〃 |
| `bootstrap_open_log` | – | ok (`logs/bootstrap.log`를 기본 앱으로 염) | – |
| `engine_health` | – | `{ running, model_loaded }` | – |
| `generate` | `{ presetId, styleId?, keyword, count, size, seed? }` | jobId | `gen://progress`, `gen://done`, `gen://error` |
| `generate_cancel` | jobId | ok | – |
| `history_list` | `{ query?, styleId?, favorite?, cursor? }` | `Generation[]` | – |
| `history_toggle_favorite` | id | ok | – |
| `export_image` | `{ id, format, transparent?, destDir }` | path | – |
| `presets_get / presets_save` | – / Preset | Preset[] / ok | – |
| `presets_export / presets_import` | `{ destPath }` / `{ srcPath }` | ok / `Preset[]` | – |
| `translate_keyword` | `{ keyword }` | `{ translated, source: notNeeded\|dict\|argos\|passthrough, warning? }` | – |
| `essence_create` | `{ imagePaths[] }` | `{ essencePrompt, tags[] }` | `essence://progress` |
| `style_save / style_delete / styles_list` | Style / id / – | ok / ok / Style[] | – |
| `kohya_install_status / kohya_install_run` | – / – | `{ installed }` / jobId | `train://install_progress` |
| `dataset_create` | `{ imagePaths[] }` | `{ id, dir, files[] }` | – |
| `caption_dataset` | `{ dir }` | `{ file, caption }[]` | `caption://progress` |
| `dataset_save_captions` | `{ dir, items: { file, caption }[] }` | ok | – |
| `training_start` | `{ styleId, datasetDir, profile: fast|standard|quality, triggerWord }` | jobId | `train://progress`, `train://sample`, `train://done`, `train://error` |
| `training_cancel` | jobId | ok | – |
| `models_list / model_download / model_set_dir` | … | … | `download://progress` |

프론트는 `src/lib/tauri.ts`에서 위 계약을 타입으로 고정하고, 스토어는 이벤트를 구독해 상태 갱신.

- A/B 비교(04 §4.4, T5.2)는 별도 커맨드 없이 `generate`를 프론트에서 두 번(프리셋 A/B, 동일 시드) 호출해 구현한다 — 두 세션 모두 `gen://*` 이벤트를 jobId로 필터링.
- `presets_export`/`presets_import`의 destPath/srcPath는 프론트가 `@tauri-apps/plugin-dialog`로 고른 절대 경로. import는 `schemaVersion`이 `presets_get`이 로드하는 파일과 다르면(§3.2, 현재 1) 아무 것도 바꾸지 않고 실패하며, 통과하면 `presets_save`와 같은 버전 관리 경로(history 스냅샷 + version+1)로 병합한다.
- kohya sd-scripts(학습 엔진)는 §7의 7단계 필수 부트스트랩에 포함하지 않는다 — LoRA 학습 화면을 처음 쓸 때 `kohya_install_run`으로 지연 설치한다(T6.1, `bootstrap::kohya`). uv/venv는 메인 부트스트랩이 준비해둔 것을 재사용하고, 로그도 같은 `logs/bootstrap.log`에 남는다.
- `training_start.triggerWord`(T6.3): kohya 폴더 규약 `{repeats}_{trigger}`와 완료 시 스타일 등록(T6.4)에 필요. styleId는 프론트가 미리 발급한 uuid — styles.json 등록 자체는 `train://done`을 받은 뒤 T6.4 경로가 수행한다. 취소·실패는 `train://error`(취소는 `E_CANCELED`)로 통일 — gen://와 같은 패턴.

## 6. ComfyUI 연동 스펙 (engine/ 모듈)

- 기동: 앱 시작 시 `runtime/venv/bin/python runtime/comfyui/main.py --listen 127.0.0.1 --port 8188` 서브프로세스 실행, `/system_stats` 폴링으로 헬스체크. 앱 종료 시 kill
- 생성: `resources/workflows/`의 템플릿 JSON을 로드해 노드 값 치환 → `POST /prompt` → WebSocket(`/ws`)으로 진행률·완료 수신 → 출력 이미지를 `outputs/`로 이동 후 DB 기록
- 워크플로 템플릿 3종(필수):
  1. `sdxl_base.json` — txt2img 기본
  2. `sdxl_ipadapter.json` — 에센스 스타일 (IP-Adapter 노드 포함, weight 파라미터화)
  3. `sdxl_lora.json` — LoRA 로더 포함 (경로·weight 파라미터화)
  - 치환 대상 노드 ID는 템플릿 내 `"_slot": "prompt|negative|seed|lora_path|..."` 커스텀 키로 표시해 코드가 슬롯 기반으로 주입
- OOM 대응: ComfyUI 에러/프로세스 사망 감지 시 ① 해상도 한 단계 하향 ② SD1.5 폴백 순으로 1회 자동 재시도, UI에 고지

## 7. 부트스트랩 (최초 실행 설치기)

상태 머신: `check → install_python(uv) → clone_comfyui → pip_install → download_models → warmup → ready`
- 각 단계 재개 가능(중단 지점 기록), 다운로드는 이어받기(HTTP Range)
- 모델 프로파일: `standard`(SDXL+IP-Adapter, ~10GB) / `light`(SD1.5, ~4GB) — RAM 감지로 기본값 제안
- 전 과정 로그를 `logs/bootstrap.log`에 기록, 실패 시 로그 열기 버튼

## 8. 학습 파이프라인 (training/ 모듈)

1. 데이터셋 준비(T6.2, D-009): `datasets/{id}/`에 평평하게 이미지 복사, `caption.py`로 .txt 캡션 생성 → UI에서 편집 → 저장
2. 학습 시작(T6.3): `training_start`가 데이터셋을 kohya 규약 `training/{jobId}/img/{repeat}_{trigger}/`로 복사 (repeat는 프로파일에서, trigger는 인자에서)
3. kohya `sdxl_train_network.py` 실행, 프로파일별 인자 프리셋(fast/standard/quality)은 `resources/training/profiles.toml`에 정의
4. stdout/stderr의 tqdm step/loss/ETA 파싱(`training/parser.rs`) → `train://progress` 이벤트, epoch마다 샘플 생성 → `train://sample`. 원문 로그는 `training/{jobId}/train.log`
5. 완료 시 .safetensors를 `models/loras/`로 이동(T6.3) → `train://done`의 loraPath로 styles.json 등록(T6.4)

## 9. 에러 처리·로깅 규약

- Rust: `thiserror`로 AppError 정의, 모든 command 경계에서 변환. 사용자 메시지는 한국어, detail에 원문
- 프론트: 전역 토스트로 AppError 표시, 재시도 가능한 작업은 재시도 버튼 포함
- 로그: `tracing` → `logs/app.log` (회전, 최대 10MB×5)
- 원칙: **어떤 에러도 이미지·프롬프트를 외부로 전송하지 않음** (크래시 리포터 미사용)

## 10. 테스트 전략 (무료 도구만)

- Rust 유닛: 프롬프트 조립, 워크플로 슬롯 치환, kohya stdout 파서 — `cargo test`
- 프론트: Vitest + Testing Library — 스토어 로직, 컴포넌트 스모크
- E2E(수동 체크리스트): docs/05 백로그의 각 태스크 AC를 그대로 사용
- CI: GitHub Actions 무료 티어 — lint + 유닛 테스트 (GPU 필요한 생성 테스트는 로컬 수동)
