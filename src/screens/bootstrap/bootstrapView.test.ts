import { describe, expect, it } from "vitest";
import type { BootstrapStep } from "../../lib/tauri";
import {
  isResume,
  progressPercent,
  PROFILE_CARDS,
  UI_STEPS,
  uiStepIndex,
  uiStepStatuses,
} from "./bootstrapView";

const ALL_STEPS: BootstrapStep[] = [
  "check",
  "install_python",
  "clone_comfyui",
  "pip_install",
  "download_models",
  "warmup",
  "ready",
];

describe("uiStepIndex", () => {
  it("7개 백엔드 단계를 04 §4.6의 4단계로 접는다", () => {
    expect(uiStepIndex("check")).toBe(0);
    expect(uiStepIndex("install_python")).toBe(0);
    expect(uiStepIndex("clone_comfyui")).toBe(1);
    expect(uiStepIndex("pip_install")).toBe(1);
    expect(uiStepIndex("download_models")).toBe(2);
    expect(uiStepIndex("warmup")).toBe(3);
    expect(uiStepIndex("ready")).toBe(3);
  });

  it("단계가 진행되면 UI 인덱스는 되돌아가지 않는다", () => {
    const indices = ALL_STEPS.map(uiStepIndex);
    for (let i = 1; i < indices.length; i++) {
      expect(indices[i]).toBeGreaterThanOrEqual(indices[i - 1]);
    }
    expect(Math.max(...indices)).toBe(UI_STEPS.length - 1);
  });
});

describe("uiStepStatuses", () => {
  it("현재 단계 앞은 done, 현재는 active, 뒤는 pending", () => {
    expect(uiStepStatuses("download_models")).toEqual(["done", "done", "active", "pending"]);
  });

  it("check(시작 전)는 첫 단계만 active", () => {
    expect(uiStepStatuses("check")).toEqual(["active", "pending", "pending", "pending"]);
  });

  it("ready면 전부 done", () => {
    expect(uiStepStatuses("ready")).toEqual(["done", "done", "done", "done"]);
  });
});

describe("progressPercent", () => {
  it("0.0~1.0을 0~100 정수로 변환한다", () => {
    expect(progressPercent(0)).toBe(0);
    expect(progressPercent(0.335)).toBe(34);
    expect(progressPercent(1)).toBe(100);
  });

  it("범위 밖 값은 클램프한다", () => {
    expect(progressPercent(-0.1)).toBe(0);
    expect(progressPercent(1.5)).toBe(100);
  });
});

describe("PROFILE_CARDS", () => {
  it("TAD §7의 두 프로파일을 모두 포함한다", () => {
    expect(PROFILE_CARDS.map((c) => c.profile)).toEqual(["standard", "light"]);
  });
});

describe("isResume", () => {
  it("중간 단계에서 중단된 경우만 true", () => {
    expect(isResume("check")).toBe(false);
    expect(isResume("ready")).toBe(false);
    expect(isResume("pip_install")).toBe(true);
    expect(isResume("download_models")).toBe(true);
  });
});
