import { describe, expect, it } from "vitest";
import type { Preset } from "../../lib/tauri";
import { isFormValid, toFormValues, toSavePayload } from "./presetForm";

function preset(overrides: Partial<Preset> = {}): Preset {
  return {
    id: "storybook",
    label: { ko: "동화같은", en: "Storybook" },
    version: 2,
    history: [
      {
        version: 1,
        label: { ko: "동화같은", en: "Storybook" },
        successCriteria: "old",
        prefix: "old prefix",
        suffix: "old suffix",
        negative: "old negative",
        params: { steps: 20, cfg: 6, width: 1024, height: 1024 },
        savedAt: 1000,
      },
    ],
    successCriteria: "파스텔톤 유지",
    prefix: "cinematic illustration of",
    suffix: "soft pastel colors",
    negative: "text, watermark",
    params: { steps: 28, cfg: 6.5, width: 1024, height: 1024 },
    ...overrides,
  };
}

describe("toFormValues", () => {
  it("flattens preset fields into form values", () => {
    const form = toFormValues(preset());
    expect(form).toEqual({
      labelKo: "동화같은",
      labelEn: "Storybook",
      successCriteria: "파스텔톤 유지",
      prefix: "cinematic illustration of",
      suffix: "soft pastel colors",
      negative: "text, watermark",
      steps: 28,
      cfg: 6.5,
      width: 1024,
      height: 1024,
    });
  });

  it("flattens a history snapshot the same way (복원 미리보기)", () => {
    const form = toFormValues(preset().history[0]);
    expect(form.prefix).toBe("old prefix");
    expect(form.steps).toBe(20);
  });
});

describe("toSavePayload", () => {
  it("merges form edits back into the base preset, keeping id", () => {
    const base = preset();
    const form = toFormValues(base);
    form.prefix = "new prefix";
    const payload = toSavePayload(base, form);
    expect(payload.id).toBe("storybook");
    expect(payload.prefix).toBe("new prefix");
    expect(payload.label).toEqual({ ko: "동화같은", en: "Storybook" });
  });
});

describe("isFormValid", () => {
  it("rejects empty prefix/negative or non-positive steps", () => {
    const valid = toFormValues(preset());
    expect(isFormValid(valid)).toBe(true);
    expect(isFormValid({ ...valid, prefix: "  " })).toBe(false);
    expect(isFormValid({ ...valid, negative: "" })).toBe(false);
    expect(isFormValid({ ...valid, steps: 0 })).toBe(false);
  });
});
