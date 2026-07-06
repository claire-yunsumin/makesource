import type { AppError } from "../../lib/appError";
import type { GenDoneEvent, GenErrorEvent, GenProgressEvent } from "../../lib/tauri";

/**
 * 생성 세션 상태 머신 (04 §4.1 상태 3종: idle / generating / error).
 *
 * 완료된 결과는 idle 상태에서 images로 표시한다. 모든 전이는 순수 함수 —
 * gen:// 이벤트는 jobId가 현재 세션과 일치할 때만 반영한다(늦게 도착한
 * 이전 잡 이벤트 무시).
 */

export type GenPhase = "idle" | "generating" | "error";

/** 완료된 결과 이미지 한 장 (T2.4 셀 액션용 id·♥ 포함). */
export interface GenImage {
  /** generations DB id */
  id: string;
  /** 데이터 루트 기준 상대 경로 */
  path: string;
  favorite: boolean;
}

export interface GenSession {
  phase: GenPhase;
  /** 진행 중인 잡. generating일 때만 non-null */
  jobId: string | null;
  /** Esc/취소 클릭 후 백엔드 확인(E_CANCELED) 대기 중 */
  cancelRequested: boolean;
  /** 0.0 ~ 1.0 */
  progress: number;
  /** OOM 폴백 등 사용자 고지 (T1.5) — 다음 생성 시작 시 초기화 */
  notice: string | null;
  /** 마지막 완료 결과 */
  images: GenImage[];
  /** 마지막 완료 배치의 시드 (시드 고정 재생성 — F-1.5) */
  seed: number | null;
  /** generating 중 그리드에 표시할 셀 수 (요청 장수) */
  cells: number;
  error: AppError | null;
}

export const INITIAL_SESSION: GenSession = {
  phase: "idle",
  jobId: null,
  cancelRequested: false,
  progress: 0,
  notice: null,
  images: [],
  seed: null,
  cells: 0,
  error: null,
};

/** 생성 시작. 이전 결과 이미지·시드는 완료 전까지 유지한다. */
export function startSession(session: GenSession, jobId: string, count: number): GenSession {
  return {
    ...INITIAL_SESSION,
    images: session.images,
    seed: session.seed,
    phase: "generating",
    jobId,
    cells: count,
  };
}

export function requestCancel(session: GenSession): GenSession {
  if (session.phase !== "generating") return session;
  return { ...session, cancelRequested: true };
}

export function applyProgress(session: GenSession, event: GenProgressEvent): GenSession {
  if (session.phase !== "generating" || session.jobId !== event.jobId) return session;
  return { ...session, progress: event.progress, notice: event.notice ?? session.notice };
}

export function applyDone(session: GenSession, event: GenDoneEvent): GenSession {
  if (session.jobId !== event.jobId) return session;
  return {
    ...session,
    phase: "idle",
    jobId: null,
    cancelRequested: false,
    progress: 0,
    images: event.imagePaths.map((path, i) => ({
      id: event.generationIds[i] ?? "",
      path,
      favorite: false,
    })),
    seed: event.seed,
    cells: 0,
    error: null,
  };
}

/** ♥ 토글 (T2.4). 낙관적 갱신 — invoke 실패 시 같은 함수로 되돌린다. */
export function toggleFavorite(session: GenSession, id: string): GenSession {
  return {
    ...session,
    images: session.images.map((img) =>
      img.id === id ? { ...img, favorite: !img.favorite } : img,
    ),
  };
}

export function applyError(session: GenSession, event: GenErrorEvent): GenSession {
  if (session.jobId !== event.jobId) return session;
  // 사용자가 취소한 경우: 에러가 아니라 조용한 idle 복귀 (이전 결과 유지)
  if (event.error.code === "E_CANCELED") {
    return {
      ...session,
      phase: "idle",
      jobId: null,
      cancelRequested: false,
      progress: 0,
      cells: 0,
    };
  }
  return {
    ...session,
    phase: "error",
    jobId: null,
    cancelRequested: false,
    progress: 0,
    cells: 0,
    error: event.error,
  };
}

/** invoke 자체가 실패한 경우 (엔진 미기동 등 — 이벤트 없이 Promise 거부). */
export function failLocal(session: GenSession, error: AppError): GenSession {
  return {
    ...session,
    phase: "error",
    jobId: null,
    cancelRequested: false,
    progress: 0,
    cells: 0,
    error,
  };
}

/** 에러 배너 닫기 → idle 복귀. */
export function dismissError(session: GenSession): GenSession {
  if (session.phase !== "error") return session;
  return { ...session, phase: "idle", error: null };
}

/**
 * 고급 패널 시드 입력 파싱. 빈칸 = 랜덤(undefined), 유효한 정수만 허용.
 * @returns undefined(랜덤) | number(고정 시드) | null(유효하지 않은 입력)
 */
export function parseSeed(input: string): number | undefined | null {
  const trimmed = input.trim();
  if (trimmed === "") return undefined;
  if (!/^-?\d+$/.test(trimmed)) return null;
  const value = Number(trimmed);
  return Number.isSafeInteger(value) ? value : null;
}
