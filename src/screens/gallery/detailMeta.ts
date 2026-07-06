/**
 * 상세 모달 메타 표시·복사·재생성 헬퍼 (T3.2, 04 §4.2).
 */
import type { Generation } from "../../lib/tauri";
import { SIZE_OPTIONS } from "../generate/presetTypes";

/** 클립보드 복사용 전체 메타 텍스트. */
export function metaText(item: Generation): string {
  const lines = [
    `프롬프트: ${item.promptFinal}`,
    item.negative ? `네거티브: ${item.negative}` : null,
    `시드: ${item.seed}`,
    item.steps !== null && item.cfg !== null ? `steps ${item.steps} · cfg ${item.cfg}` : null,
    item.width !== null && item.height !== null ? `크기: ${item.width}×${item.height}` : null,
    item.presetId ? `프리셋: ${item.presetId} v${item.presetVersion ?? 1}` : null,
    item.model ? `모델: ${item.model}` : null,
  ];
  return lines.filter((l): l is string => l !== null).join("\n");
}

/** "같은 설정으로 다시 생성"이 생성 화면 폼에 채울 값 (04 §4.2). */
export function regenFormState(item: Generation): {
  presetId: string;
  keyword: string;
  seedInput: string;
  sizeIndex: number;
} {
  const sizeIndex = SIZE_OPTIONS.findIndex(
    (opt) => opt.size[0] === item.width && opt.size[1] === item.height,
  );
  return {
    presetId: item.presetId ?? "",
    keyword: item.keywordKo ?? "",
    seedInput: String(item.seed),
    // 히스토리의 크기가 현재 옵션에 없으면(폴백 하향 등) 기본 1:1로
    sizeIndex: sizeIndex >= 0 ? sizeIndex : 0,
  };
}
