/**
 * 고급 패널 변환·프롬프트 미리보기 순수 헬퍼 (T2.3).
 * 실제 변환은 백엔드(translate_keyword)가 수행 — 여기서는 표시 로직만.
 */
import type { Preset, TranslationSource } from "../../lib/tauri";

/** 한글(완성형 음절 + 자모) 포함 여부 — 백엔드 contains_hangul과 같은 범위. */
export function containsHangul(text: string): boolean {
  return /[가-힣ᄀ-ᇿ㄰-㆏]/.test(text);
}

/** 변환 경로 표시 라벨 (04 §6: 기술 용어 최소화). notNeeded는 표시 없음. */
export function translationSourceLabel(source: TranslationSource): string | null {
  switch (source) {
    case "dict":
      return "용어 사전";
    case "argos":
      return "자동 번역";
    case "passthrough":
      return "원문 사용";
    case "notNeeded":
      return null;
  }
}

/**
 * 조립될 프롬프트 미리보기 — 백엔드 assemble_prompt(TAD §4)와 같은 규칙:
 * prefix, 키워드, 에센스, suffix 순 (빈 조각 생략). LoRA 트리거워드는 M6에서.
 */
export function previewPrompt(preset: Preset, keywordEn: string, essencePrompt = ""): string {
  return [preset.prefix, keywordEn, essencePrompt, preset.suffix]
    .map((part) => part.trim())
    .filter((part) => part !== "")
    .join(", ");
}
