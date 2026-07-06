import { describe, expect, it } from "vitest";
import type { Preset } from "../../lib/tauri";
import { presetLabel, resolvePresetId } from "./presetTypes";

function makePreset(id: string, label: Preset["label"]): Preset {
  return {
    id,
    label,
    version: 1,
    history: [],
    successCriteria: "",
    prefix: "",
    suffix: "",
    negative: "",
    params: { steps: 20, cfg: 6, width: 1024, height: 1024 },
  };
}

describe("presetLabel", () => {
  it("현재 언어 라벨을 우선 사용한다", () => {
    const p = makePreset("storybook", { ko: "동화같은", en: "Storybook" });
    expect(presetLabel(p, "ko")).toBe("동화같은");
    expect(presetLabel(p, "en")).toBe("Storybook");
  });

  it("해당 언어가 없으면 다른 언어로 폴백한다", () => {
    const p = makePreset("storybook", { en: "Storybook" });
    expect(presetLabel(p, "ko")).toBe("Storybook");
  });

  it("라벨이 비면 id로 폴백한다", () => {
    expect(presetLabel(makePreset("mine", {}), "ko")).toBe("mine");
  });

  it("기본 언어는 ko다", () => {
    const p = makePreset("storybook", { ko: "동화같은", en: "Storybook" });
    expect(presetLabel(p)).toBe("동화같은");
  });
});

describe("resolvePresetId", () => {
  const presets = [makePreset("a", {}), makePreset("b", {})];

  it("현재 선택이 목록에 있으면 유지한다", () => {
    expect(resolvePresetId(presets, "b")).toBe("b");
  });

  it("현재 선택이 목록에 없으면 첫 항목으로 정합화한다", () => {
    expect(resolvePresetId(presets, "gone")).toBe("a");
    expect(resolvePresetId(presets, "")).toBe("a");
  });

  it("목록이 비면 빈 문자열", () => {
    expect(resolvePresetId([], "a")).toBe("");
  });
});
