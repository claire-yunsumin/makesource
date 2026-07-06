/**
 * 이미지 타입(프리셋) 선택 UI 헬퍼 + 화면 상수.
 *
 * 프리셋 자체는 presets_get(백엔드, resources/presets.default.json)에서 로딩한다.
 * 여기서는 로딩된 프리셋을 카드로 표시할 때 쓰는 순수 헬퍼와, 백엔드와 무관한
 * UI 상수(크기·장수)만 둔다.
 */
import type { Preset } from "../../lib/tauri";

/** 프리셋 라벨을 현재 언어로. 해당 언어가 없으면 다른 언어 → id 순으로 폴백. */
export function presetLabel(preset: Preset, lang: "ko" | "en" = "ko"): string {
  const other = lang === "ko" ? "en" : "ko";
  return preset.label?.[lang] || preset.label?.[other] || preset.id;
}

/**
 * 선택 상태를 프리셋 목록과 정합화한다.
 * 현재 선택이 목록에 있으면 유지, 없으면 첫 프리셋(목록이 비면 "")으로.
 */
export function resolvePresetId(presets: Preset[], current: string): string {
  if (presets.some((p) => p.id === current)) return current;
  return presets[0]?.id ?? "";
}

/** 크기 3옵션 (04 §4.1) — SDXL 권장 해상도 버킷. */
export const SIZE_OPTIONS: { label: string; size: [number, number] }[] = [
  { label: "정방형 1:1", size: [1024, 1024] },
  { label: "가로 3:2", size: [1216, 832] },
  { label: "세로 2:3", size: [832, 1216] },
];

export const COUNT_OPTIONS = [1, 2, 3, 4];
