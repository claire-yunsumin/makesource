import { describe, expect, it } from "vitest";
import type { Preset } from "../../lib/tauri";
import { containsHangul, previewPrompt, translationSourceLabel } from "./translationPreview";

describe("containsHangul", () => {
  it("완성형 음절과 자모를 감지한다", () => {
    expect(containsHangul("통나무집")).toBe(true);
    expect(containsHangul("red 자동차")).toBe(true);
    expect(containsHangul("ㅋㅋ")).toBe(true);
  });

  it("한글이 없으면 false", () => {
    expect(containsHangul("log cabin")).toBe(false);
    expect(containsHangul("123 !@#")).toBe(false);
    expect(containsHangul("")).toBe(false);
  });
});

describe("translationSourceLabel", () => {
  it("경로별 라벨, notNeeded는 null", () => {
    expect(translationSourceLabel("dict")).toBe("용어 사전");
    expect(translationSourceLabel("argos")).toBe("자동 번역");
    expect(translationSourceLabel("passthrough")).toBe("원문 사용");
    expect(translationSourceLabel("notNeeded")).toBeNull();
  });
});

describe("previewPrompt", () => {
  const preset = {
    id: "storybook",
    label: { ko: "동화같은" },
    version: 1,
    history: [],
    successCriteria: "",
    prefix: "cinematic illustration of",
    suffix: "soft pastel colors",
    negative: "",
    params: { steps: 28, cfg: 6.5, width: 1024, height: 1024 },
  } satisfies Preset;

  it("prefix, 키워드, suffix를 쉼표로 잇는다 (백엔드 assemble_prompt 규칙)", () => {
    expect(previewPrompt(preset, "log cabin")).toBe(
      "cinematic illustration of, log cabin, soft pastel colors",
    );
  });

  it("빈 조각은 건너뛴다", () => {
    expect(previewPrompt({ ...preset, suffix: "  " }, "cabin")).toBe(
      "cinematic illustration of, cabin",
    );
  });
});
