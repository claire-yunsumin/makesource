import type { BootstrapStep, ModelProfile } from "../../lib/tauri";

/**
 * 부트스트랩 화면(04 §4.6) 표시 로직.
 * 백엔드 상태 머신의 7단계(TAD §7)를 사용자에게 보여줄 4단계로 접는다:
 * 환경 준비 → 엔진 설치 → 모델 다운로드 → 준비 완료
 */

export interface UiStep {
  label: string;
}

export const UI_STEPS: UiStep[] = [
  { label: "환경 준비" },
  { label: "엔진 설치" },
  { label: "모델 다운로드" },
  { label: "준비 완료" },
];

/** 백엔드 단계 → UI 단계 인덱스 (UI_STEPS 기준). */
export function uiStepIndex(step: BootstrapStep): number {
  switch (step) {
    case "check":
    case "install_python":
      return 0;
    case "clone_comfyui":
    case "pip_install":
      return 1;
    case "download_models":
      return 2;
    case "warmup":
    case "ready":
      return 3;
  }
}

export type UiStepStatus = "done" | "active" | "pending";

/** 현재 백엔드 단계 기준으로 각 UI 단계의 표시 상태. ready면 전부 done. */
export function uiStepStatuses(step: BootstrapStep): UiStepStatus[] {
  const active = uiStepIndex(step);
  return UI_STEPS.map((_, i) => {
    if (step === "ready") return "done";
    if (i < active) return "done";
    if (i === active) return "active";
    return "pending";
  });
}

/** 진행률(0.0~1.0) → 표시용 퍼센트 정수(0~100, 범위 밖 값은 클램프). */
export function progressPercent(progress: number): number {
  const clamped = Math.min(1, Math.max(0, progress));
  return Math.round(clamped * 100);
}

export interface ProfileCard {
  profile: ModelProfile;
  title: string;
  description: string;
  size: string;
}

/** 프로파일 선택 카드 (TAD §7: standard ~10GB / light ~4GB). */
export const PROFILE_CARDS: ProfileCard[] = [
  {
    profile: "standard",
    title: "표준",
    description: "고품질 SDXL 모델과 에센스 스타일까지 모두 사용해요.",
    size: "약 10GB",
  },
  {
    profile: "light",
    title: "라이트",
    description: "가벼운 SD1.5 모델로 빠르게 시작해요. 메모리가 적어도 괜찮아요.",
    size: "약 4GB",
  },
];

/**
 * 중단된 설치를 이어서 하는 상황인지 (04 §4.6 — 버튼 문구 분기).
 * check는 아직 아무것도 안 한 상태, ready는 이미 끝난 상태라 둘 다 아님.
 */
export function isResume(step: BootstrapStep): boolean {
  return step !== "check" && step !== "ready";
}
