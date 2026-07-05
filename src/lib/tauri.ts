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
