import { create } from "zustand";
import type { AppError } from "../../lib/appError";
import type { GenDoneEvent, GenErrorEvent, GenProgressEvent } from "../../lib/tauri";
import {
  INITIAL_SESSION,
  applyDone,
  applyError,
  applyProgress,
  dismissError,
  failLocal,
  requestCancel,
  startSession,
  type GenSession,
} from "./genSession";
import { DEFAULT_PRESET_ID } from "./presetTypes";

/** 생성 화면 로컬 스토어 (폼 입력 + 세션). 전이 로직은 genSession의 순수 함수에 위임. */
interface GenerateState {
  presetId: string;
  keyword: string;
  count: number;
  /** SIZE_OPTIONS 인덱스 */
  sizeIndex: number;
  /** 고급 패널 시드 입력 원문 (빈칸 = 랜덤) */
  seedInput: string;
  session: GenSession;

  setPresetId: (id: string) => void;
  setKeyword: (keyword: string) => void;
  setCount: (count: number) => void;
  setSizeIndex: (index: number) => void;
  setSeedInput: (input: string) => void;

  start: (jobId: string, count: number) => void;
  onProgress: (event: GenProgressEvent) => void;
  onDone: (event: GenDoneEvent) => void;
  onError: (event: GenErrorEvent) => void;
  failLocal: (error: AppError) => void;
  markCancelRequested: () => void;
  dismissError: () => void;
}

export const useGenerateStore = create<GenerateState>((set) => ({
  presetId: DEFAULT_PRESET_ID,
  keyword: "",
  count: 4,
  sizeIndex: 0,
  seedInput: "",
  session: INITIAL_SESSION,

  setPresetId: (presetId) => set({ presetId }),
  setKeyword: (keyword) => set({ keyword }),
  setCount: (count) => set({ count }),
  setSizeIndex: (sizeIndex) => set({ sizeIndex }),
  setSeedInput: (seedInput) => set({ seedInput }),

  start: (jobId, count) => set((s) => ({ session: startSession(s.session, jobId, count) })),
  onProgress: (event) => set((s) => ({ session: applyProgress(s.session, event) })),
  onDone: (event) => set((s) => ({ session: applyDone(s.session, event) })),
  onError: (event) => set((s) => ({ session: applyError(s.session, event) })),
  failLocal: (error) => set((s) => ({ session: failLocal(s.session, error) })),
  markCancelRequested: () => set((s) => ({ session: requestCancel(s.session) })),
  dismissError: () => set((s) => ({ session: dismissError(s.session) })),
}));
