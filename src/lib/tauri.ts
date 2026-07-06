import { invoke } from "@tauri-apps/api/core";
import type { AppError } from "./appError";

export type { AppError };

/**
 * 타입 안전 invoke 래퍼. TAD §5의 Tauri command 계약을 이 파일에서 고정한다.
 * 각 command의 입력/출력 타입은 해당 기능 태스크에서 추가한다.
 *
 * long-running 작업(generate/train/bootstrap 등)은 command가 jobId만 반환하고
 * 진행 상황은 Tauri event로 push된다 (CLAUDE.md 규칙 4).
 */
export async function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  return invoke<T>(command, args);
}

// ---- bootstrap (TAD §5, §7) ----

export type ModelProfile = "standard" | "light";

export type BootstrapStep =
  | "check"
  | "install_python"
  | "clone_comfyui"
  | "pip_install"
  | "download_models"
  | "warmup"
  | "ready";

export interface BootstrapStatus {
  step: BootstrapStep;
  progress: number;
  ready: boolean;
  suggestedProfile: ModelProfile;
}

/** `bootstrap://progress` 이벤트 페이로드 */
export interface BootstrapProgressEvent {
  step: BootstrapStep;
  progress: number;
  message: string;
  error?: string;
}

export const BOOTSTRAP_PROGRESS_EVENT = "bootstrap://progress";

export function bootstrapStatus(): Promise<BootstrapStatus> {
  return invokeCommand<BootstrapStatus>("bootstrap_status");
}

/** 설치 시작. jobId 반환, 진행은 BOOTSTRAP_PROGRESS_EVENT 구독. */
export function bootstrapRun(modelProfile: ModelProfile): Promise<string> {
  return invokeCommand<string>("bootstrap_run", { modelProfile });
}

/** 설치 실패 시 logs/bootstrap.log를 기본 앱으로 연다 (04 §4.6 로그 열기). */
export function bootstrapOpenLog(): Promise<void> {
  return invokeCommand<void>("bootstrap_open_log");
}

// ---- engine (TAD §5, §6) ----

export interface EngineHealth {
  running: boolean;
  modelLoaded: boolean;
}

export function engineHealth(): Promise<EngineHealth> {
  return invokeCommand<EngineHealth>("engine_health");
}

// ---- generate (TAD §5) ----

export interface GenerateArgs {
  presetId: string;
  styleId?: string;
  keyword: string;
  count?: number;
  /** [width, height] */
  size?: [number, number];
  seed?: number;
}

/** gen://progress 페이로드 */
export interface GenProgressEvent {
  jobId: string;
  progress: number;
  /** 폴백 등 사용자 고지 (예: "메모리가 부족해 크기를 낮춰 다시 시도했어요.") */
  notice?: string;
}

/** gen://done 페이로드 */
export interface GenDoneEvent {
  jobId: string;
  generationIds: string[];
  /** 앱 데이터 루트 기준 상대 경로 */
  imagePaths: string[];
  /** 이 배치에 사용된 시드 (시드 고정 재생성 — F-1.5) */
  seed: number;
}

/** gen://error 페이로드 */
export interface GenErrorEvent {
  jobId: string;
  error: AppError;
}

export const GEN_PROGRESS_EVENT = "gen://progress";
export const GEN_DONE_EVENT = "gen://done";
export const GEN_ERROR_EVENT = "gen://error";

/** 생성 시작. jobId 반환, 진행/완료/실패는 gen:// 이벤트 구독. */
export function generate(args: GenerateArgs): Promise<string> {
  return invokeCommand<string>("generate", { args });
}

export function generateCancel(jobId: string): Promise<void> {
  return invokeCommand<void>("generate_cancel", { jobId });
}

// ---- presets (TAD §5, §3.2) ----

/** 로컬화 라벨. 사용자 프리셋은 일부 언어만 채울 수 있어 partial. */
export type PresetLabel = Partial<Record<"ko" | "en", string>>;

export interface PresetParams {
  steps: number;
  cfg: number;
  width: number;
  height: number;
}

/** 저장 시점의 편집 가능 필드 스냅샷 (TAD §3.2 `history[]`). */
export interface PresetSnapshot {
  version: number;
  label: PresetLabel;
  successCriteria: string;
  prefix: string;
  suffix: string;
  negative: string;
  params: PresetParams;
  /** unix ms */
  savedAt: number;
}

export interface Preset {
  id: string;
  label: PresetLabel;
  version: number;
  /** 이전 버전 스냅샷, 최신이 [0] (T5.1) */
  history: PresetSnapshot[];
  successCriteria: string;
  prefix: string;
  suffix: string;
  negative: string;
  params: PresetParams;
}

/** 프리셋 목록 로드 (사용자 presets.json 또는 내장 기본값). */
export function presetsGet(): Promise<Preset[]> {
  return invokeCommand<Preset[]>("presets_get");
}

/**
 * 저장: 서버가 현재 상태를 history에 스냅샷으로 남기고 version을 올린다.
 * 전달한 version/history는 무시됨 — 복원도 이 함수로 처리(스냅샷 필드값을
 * 다시 저장하면 현재 버전이 history에 보존되며 새 버전으로 복원됨).
 */
export function presetsSave(preset: Preset): Promise<void> {
  return invokeCommand<void>("presets_save", { preset });
}

/** 현재 프리셋 전체를 destPath(절대 경로, 저장 다이얼로그로 선택)로 내보낸다. */
export function presetsExport(destPath: string): Promise<void> {
  return invokeCommand<void>("presets_export", { destPath });
}

/**
 * srcPath(절대 경로, 열기 다이얼로그로 선택)의 파일을 가져온다.
 * schemaVersion이 안 맞으면 아무것도 바꾸지 않고 실패한다. 성공하면
 * presets_save와 같은 버전 관리를 타 병합된 최신 프리셋 목록을 반환한다.
 */
export function presetsImport(srcPath: string): Promise<Preset[]> {
  return invokeCommand<Preset[]>("presets_import", { srcPath });
}

// ---- training / kohya 선택 설치 (TAD §5, T6.1) ----

export interface KohyaInstallStatus {
  installed: boolean;
}

/** LoRA 학습 도구(kohya sd-scripts)가 이미 설치돼 있는지 (지연 설치, 첫 사용 시). */
export function kohyaInstallStatus(): Promise<KohyaInstallStatus> {
  return invokeCommand<KohyaInstallStatus>("kohya_install_status");
}

/** `train://install_progress` 페이로드 */
export interface KohyaInstallProgressEvent {
  jobId: string;
  done: boolean;
  message: string;
  error?: AppError;
}

export const KOHYA_INSTALL_PROGRESS_EVENT = "train://install_progress";

/** 설치 시작. jobId 반환, 완료/실패는 KOHYA_INSTALL_PROGRESS_EVENT 구독. */
export function kohyaInstallRun(): Promise<string> {
  return invokeCommand<string>("kohya_install_run");
}

// ---- LoRA 학습 잡 (TAD §5/§8, T6.3) ----

export type TrainingProfile = "fast" | "standard" | "quality";

export interface TrainingStartArgs {
  /** 완료 시 등록될 스타일 id (프론트에서 미리 발급 — 등록 자체는 T6.4) */
  styleId: string;
  /** dataset_create가 돌려준 dir */
  datasetDir: string;
  profile: TrainingProfile;
  /** 폴더 규약({repeats}_{trigger})과 스타일 등록에 쓰임 */
  triggerWord: string;
}

/** `train://progress` 페이로드 */
export interface TrainProgressEvent {
  jobId: string;
  /** 0.0 ~ 1.0 */
  progress: number;
  etaSeconds?: number;
  loss?: number;
  /** [현재, 전체] — epoch 경계 이후부터 채워짐 */
  epoch?: [number, number];
}

/** `train://sample` 페이로드 (epoch 샘플 이미지, 절대 경로) */
export interface TrainSampleEvent {
  jobId: string;
  imagePath: string;
}

/** `train://done` 페이로드 */
export interface TrainDoneEvent {
  jobId: string;
  styleId: string;
  /** 데이터 루트 기준 상대 경로 (models/loras/…) */
  loraPath: string;
  triggerWord: string;
}

/** `train://error` 페이로드 (취소는 error.code === "E_CANCELED") */
export interface TrainErrorEvent {
  jobId: string;
  error: AppError;
}

export const TRAIN_PROGRESS_EVENT = "train://progress";
export const TRAIN_SAMPLE_EVENT = "train://sample";
export const TRAIN_DONE_EVENT = "train://done";
export const TRAIN_ERROR_EVENT = "train://error";

/** 학습 시작. jobId 반환, 진행/샘플/완료/실패는 train:// 이벤트 구독. */
export function trainingStart(args: TrainingStartArgs): Promise<string> {
  return invokeCommand<string>("training_start", { args });
}

export function trainingCancel(jobId: string): Promise<void> {
  return invokeCommand<void>("training_cancel", { jobId });
}

// ---- translate (TAD §4, §5) ----

/**
 * 변환 경로: notNeeded(한글 없음) → dict(용어 사전) → argos(로컬 번역) →
 * passthrough(실패, 원문 사용 + warning)
 */
export type TranslationSource = "notNeeded" | "dict" | "argos" | "passthrough";

export interface Translation {
  translated: string;
  source: TranslationSource;
  /** passthrough일 때 사용자 고지 문구 */
  warning?: string;
}

/** 한→영 변환 미리보기 (고급 패널). 생성 파이프라인은 백엔드에서 같은 로직을 직접 수행. */
export function translateKeyword(keyword: string): Promise<Translation> {
  return invokeCommand<Translation>("translate_keyword", { keyword });
}

// ---- history / export (TAD §5, T2.4/T3.1) ----

/** generations DB 행 (TAD §3.1 — Rust db::models::Generation과 동기). */
export interface Generation {
  id: string;
  /** unix ms */
  createdAt: number;
  /** 앱 데이터 루트 기준 상대 경로 */
  imagePath: string;
  thumbPath: string;
  keywordKo: string | null;
  promptFinal: string;
  negative: string | null;
  presetId: string | null;
  presetVersion: number | null;
  styleId: string | null;
  seed: number;
  steps: number | null;
  cfg: number | null;
  width: number | null;
  height: number | null;
  model: string | null;
  favorite: boolean;
}

export interface HistoryListArgs {
  /** 키워드(한글)·프롬프트(영문) 부분 일치 검색 */
  query?: string;
  styleId?: string;
  /** true = ♥만 */
  favorite?: boolean;
  /** 직전 페이지 마지막 항목의 커서 ("{createdAt}:{id}"). 없으면 첫 페이지 */
  cursor?: string;
}

/** 히스토리 최신순 페이지 (페이지 크기 40 — 백엔드 PAGE_SIZE). */
export function historyList(args?: HistoryListArgs): Promise<Generation[]> {
  return invokeCommand<Generation[]>("history_list", { args });
}

/** 즐겨찾기 토글. 프론트는 낙관적으로 갱신하고 실패 시 되돌린다. */
export function historyToggleFavorite(id: string): Promise<void> {
  return invokeCommand<void>("history_toggle_favorite", { id });
}

export interface ExportImageArgs {
  id: string;
  /** T2.4는 png만 — jpg/webp는 갤러리 상세(T3.2)에서 */
  format: "png" | "jpg" | "webp";
  transparent?: boolean;
  destDir: string;
}

/** 이미지 내보내기. 저장된 절대 경로를 돌려준다. */
export function exportImage(args: ExportImageArgs): Promise<string> {
  return invokeCommand<string>("export_image", { args });
}

// ---- styles / essence (TAD §5, §3.3 — T4.2) ----

export type StyleKind = "essence" | "lora";

/** styles.json 항목 (TAD §3.3 — Rust styles::Style과 동기). */
export interface Style {
  id: string;
  name: string;
  kind: StyleKind;
  essencePrompt?: string;
  /** 데이터 루트 기준 상대 경로 (저장 시 절대 경로는 백엔드가 복사 후 상대화) */
  referenceImages: string[];
  ipAdapterWeight?: number;
  loraPath?: string;
  loraWeight?: number;
  triggerWord?: string;
  thumb?: string;
  createdAt: number;
}

export function stylesList(): Promise<Style[]> {
  return invokeCommand<Style[]>("styles_list");
}

export function styleSave(style: Style): Promise<void> {
  return invokeCommand<void>("style_save", { style });
}

export function styleDelete(id: string): Promise<void> {
  return invokeCommand<void>("style_delete", { id });
}

export interface EssenceResult {
  essencePrompt: string;
  tags: string[];
  captions: string[];
}

/** `essence://progress` 페이로드 (분석 로그 중계) */
export interface EssenceProgressEvent {
  message: string;
}

export const ESSENCE_PROGRESS_EVENT = "essence://progress";

/**
 * 참조 이미지 3~10장 → 에센스 프롬프트 (TAD §5 — 결과를 직접 반환, 진행은 이벤트).
 * 첫 실행은 모델 다운로드로 수 분 걸릴 수 있다.
 */
export function essenceCreate(imagePaths: string[]): Promise<EssenceResult> {
  return invokeCommand<EssenceResult>("essence_create", { args: { imagePaths } });
}

// ---- LoRA 학습 데이터셋 (TAD §5, T6.2, 04 §4.3 학습 마법사 ①②) ----

export interface DatasetInfo {
  id: string;
  /** 앱 데이터 루트 기준 절대 경로 — caption_dataset/training_start(T6.3)에 그대로 넘긴다 */
  dir: string;
  /** dir 안의 이미지 파일명(경로 아님) */
  files: string[];
}

/** 드롭한 이미지(절대 경로)를 datasets/{id}/로 복사해 새 데이터셋을 만든다. */
export function datasetCreate(imagePaths: string[]): Promise<DatasetInfo> {
  return invokeCommand<DatasetInfo>("dataset_create", { imagePaths });
}

export interface CaptionItem {
  file: string;
  caption: string;
}

/** `caption://progress` 페이로드 (캡션 생성 로그 중계) */
export interface CaptionProgressEvent {
  message: string;
}

export const CAPTION_PROGRESS_EVENT = "caption://progress";

/**
 * dir 안의 이미지에 WD14 태그 캡션을 자동 생성한다(결과를 직접 반환, 진행은
 * 이벤트 — essence_create와 동일한 계약 형태). 사용자가 캡션 테이블에서
 * 다듬거나 트리거 단어를 일괄 추가한 뒤 datasetSaveCaptions로 저장한다.
 */
export function captionDataset(dir: string): Promise<CaptionItem[]> {
  return invokeCommand<CaptionItem[]>("caption_dataset", { dir });
}

/** 캡션을 kohya sd-scripts 관례({basename}.txt, 이미지와 같은 폴더)로 저장. */
export function datasetSaveCaptions(dir: string, items: CaptionItem[]): Promise<void> {
  return invokeCommand<void>("dataset_save_captions", { dir, items });
}
