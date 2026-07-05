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
