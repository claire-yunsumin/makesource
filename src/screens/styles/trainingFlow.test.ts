import { describe, expect, it } from "vitest";
import { PROFILE_OPTIONS, canStartTraining, deriveTriggerWord, formatEta } from "./trainingFlow";

describe("PROFILE_OPTIONS", () => {
  it("has the three contract profiles in order (04 §4.3 ③)", () => {
    expect(PROFILE_OPTIONS.map((p) => p.id)).toEqual(["fast", "standard", "quality"]);
    for (const p of PROFILE_OPTIONS) {
      expect(p.estimate).not.toBe("");
    }
  });
});

describe("deriveTriggerWord", () => {
  it("keeps ascii alnum lowercase, matching backend sanitize_trigger", () => {
    expect(deriveTriggerWord("MyStyle")).toBe("mystyle");
    expect(deriveTriggerWord("Brand-2 Tone")).toBe("brand2tone");
  });

  it("falls back to 'style' for non-ascii names (한글 등)", () => {
    expect(deriveTriggerWord("우리 브랜드")).toBe("style");
    expect(deriveTriggerWord("")).toBe("style");
  });
});

describe("canStartTraining", () => {
  it("requires both name and trigger word", () => {
    expect(canStartTraining("우리 브랜드", "brand")).toBe(true);
    expect(canStartTraining("  ", "brand")).toBe(false);
    expect(canStartTraining("우리 브랜드", " ")).toBe(false);
  });
});

describe("formatEta", () => {
  it("is null when unknown", () => {
    expect(formatEta(undefined)).toBeNull();
    expect(formatEta(-1)).toBeNull();
  });

  it("rounds to friendly minutes/hours (04 §6 톤)", () => {
    expect(formatEta(30)).toBe("1분 남짓 남았어요");
    expect(formatEta(300)).toBe("약 5분 남았어요");
    expect(formatEta(3600)).toBe("약 1시간 남았어요");
    expect(formatEta(3630)).toBe("약 1시간 1분 남았어요");
    expect(formatEta(7200)).toBe("약 2시간 남았어요");
  });
});
