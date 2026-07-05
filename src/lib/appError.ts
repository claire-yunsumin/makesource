/**
 * Rust 백엔드가 반환하는 통일 에러 형태 (TAD §5, §9).
 * 모든 Tauri command는 `Result<T, AppError>`를 반환한다.
 */
export interface AppError {
  code: string;
  message: string;
  detail?: string;
}

/** 임의의 값이 AppError 형태인지 판별하는 타입 가드. */
export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    "message" in value &&
    typeof (value as Record<string, unknown>).code === "string" &&
    typeof (value as Record<string, unknown>).message === "string"
  );
}
