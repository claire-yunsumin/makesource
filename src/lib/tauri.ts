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

export interface Preset {
  id: string;
  label: PresetLabel;
  version: number;
  /** 이전 버전 스냅샷 (버전 관리는 T5.1) */
  history: unknown[];
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
