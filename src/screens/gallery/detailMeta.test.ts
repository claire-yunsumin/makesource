import { describe, expect, it } from "vitest";
import type { Generation } from "../../lib/tauri";
import { metaText, regenFormState } from "./detailMeta";

const base: Generation = {
  id: "g1",
  createdAt: 1_700_000_000_000,
  imagePath: "outputs/2026-07/g1.png",
  thumbPath: "outputs/2026-07/g1.png",
  keywordKo: "통나무집",
  promptFinal: "cinematic illustration of, log cabin, soft pastel colors",
  negative: "text, watermark",
  presetId: "storybook",
  presetVersion: 1,
  styleId: null,
  seed: 42,
  steps: 28,
  cfg: 6.5,
  width: 1216,
  height: 832,
  model: "sd_xl_base_1.0.safetensors",
  favorite: false,
};

describe("metaText", () => {
  it("전체 메타를 줄 단위로 만든다", () => {
    const text = metaText(base);
    expect(text).toContain("프롬프트: cinematic illustration of");
    expect(text).toContain("네거티브: text, watermark");
    expect(text).toContain("시드: 42");
    expect(text).toContain("steps 28 · cfg 6.5");
    expect(text).toContain("크기: 1216×832");
    expect(text).toContain("프리셋: storybook v1");
  });

  it("없는 필드 줄은 생략한다", () => {
    const text = metaText({ ...base, negative: null, model: null, steps: null });
    expect(text).not.toContain("네거티브");
    expect(text).not.toContain("모델");
    expect(text).not.toContain("steps");
  });
});

describe("regenFormState", () => {
  it("프리셋·키워드·시드·크기를 폼 상태로 만든다", () => {
    const form = regenFormState(base);
    expect(form).toEqual({
      presetId: "storybook",
      keyword: "통나무집",
      seedInput: "42",
      sizeIndex: 1, // 1216×832 = 가로 3:2
    });
  });

  it("현재 크기 옵션에 없는 해상도(폴백 하향)는 기본 1:1로", () => {
    expect(regenFormState({ ...base, width: 512, height: 512 }).sizeIndex).toBe(0);
    expect(regenFormState({ ...base, width: null, height: null }).sizeIndex).toBe(0);
  });

  it("키워드·프리셋이 없으면 빈 문자열", () => {
    const form = regenFormState({ ...base, keywordKo: null, presetId: null });
    expect(form.keyword).toBe("");
    expect(form.presetId).toBe("");
  });
});
