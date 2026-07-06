import { create } from "zustand";

/**
 * 온보딩 투어 전역 상태 (T8.1). Layout이 최초 실행 시 start()하고,
 * 설정 화면의 [가이드 다시 보기]도 start()를 부른다.
 * "봤다" 표시는 webview localStorage — 잃어버려도 투어가 한 번 더 나올 뿐이라
 * settings.json(IPC 계약)까지 늘리지 않았다.
 */

const STORAGE_KEY = "localbrush.tour.v1";

export function hasSeenTour(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === "done";
  } catch {
    return true; // 저장소를 못 쓰는 환경이면 투어를 반복 노출하지 않는 쪽으로
  }
}

export function markTourSeen(): void {
  try {
    localStorage.setItem(STORAGE_KEY, "done");
  } catch {
    // 실패해도 치명적이지 않음 — 다음 실행에 투어가 한 번 더 보일 뿐
  }
}

interface OnboardingState {
  active: boolean;
  stepIndex: number;
  start: () => void;
  goTo: (index: number) => void;
  /** 건너뛰기·완료 공통 — "봤다"로 기록하고 닫는다 */
  finish: () => void;
}

export const useOnboarding = create<OnboardingState>((set) => ({
  active: false,
  stepIndex: 0,
  start: () => set({ active: true, stepIndex: 0 }),
  goTo: (index) => set({ stepIndex: index }),
  finish: () => {
    markTourSeen();
    set({ active: false, stepIndex: 0 });
  },
}));
