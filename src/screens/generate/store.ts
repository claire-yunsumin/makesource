import { create } from "zustand";
import type { AppError } from "../../lib/appError";
import type { GenDoneEvent, GenErrorEvent, GenProgressEvent, Preset, Style } from "../../lib/tauri";
import {
  INITIAL_SESSION,
  applyDone,
  applyError,
  applyProgress,
  dismissError,
  failLocal,
  requestCancel,
  startSession,
  toggleFavorite,
  type GenSession,
} from "./genSession";
import { resolvePresetId } from "./presetTypes";

/** 생성 화면 로컬 스토어 (폼 입력 + 세션). 전이 로직은 genSession의 순수 함수에 위임. */
interface GenerateState {
  /** 로딩된 이미지 타입 프리셋 (presets_get) */
  presets: Preset[];
  presetsLoading: boolean;
  presetsError: AppError | null;
  /** 선택된 프리셋 id (프리셋 로딩 전에는 "") */
  presetId: string;
  /** 로딩된 스타일 목록 (styles_list — T4.3) */
  styles: Style[];
  /** 선택된 스타일 id ("" = 없음) */
  styleId: string;
  keyword: string;
  count: number;
  /** SIZE_OPTIONS 인덱스 */
  sizeIndex: number;
  /** 고급 패널 시드 입력 원문 (빈칸 = 랜덤) */
  seedInput: string;
  session: GenSession;

  setPresets: (presets: Preset[]) => void;
  setPresetsError: (error: AppError | null) => void;
  setPresetId: (id: string) => void;
  /** 목록 저장 + 선택 정합화 (선택된 스타일이 삭제됐으면 "없음"으로) */
  setStyles: (styles: Style[]) => void;
  setStyleId: (id: string) => void;
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
  /** ♥ 낙관적 토글 (실패 시 한 번 더 호출해 되돌림) */
  toggleFavorite: (id: string) => void;
}

export const useGenerateStore = create<GenerateState>((set) => ({
  presets: [],
  presetsLoading: true,
  presetsError: null,
  presetId: "",
  styles: [],
  styleId: "",
  keyword: "",
  count: 4,
  sizeIndex: 0,
  seedInput: "",
  session: INITIAL_SESSION,

  // 로딩 성공: 목록 저장 + 선택 정합화(기존 선택 유지, 없으면 첫 항목)
  setPresets: (presets) =>
    set((s) => ({
      presets,
      presetsLoading: false,
      presetsError: null,
      presetId: resolvePresetId(presets, s.presetId),
    })),
  setPresetsError: (presetsError) => set({ presetsError, presetsLoading: false }),
  setPresetId: (presetId) => set({ presetId }),
  setStyles: (styles) =>
    set((s) => ({
      styles,
      styleId: styles.some((st) => st.id === s.styleId) ? s.styleId : "",
    })),
  setStyleId: (styleId) => set({ styleId }),
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
  toggleFavorite: (id) => set((s) => ({ session: toggleFavorite(s.session, id) })),
}));
