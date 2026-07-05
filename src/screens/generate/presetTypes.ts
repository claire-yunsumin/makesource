/**
 * 이미지 타입(프리셋) 선택 카드 데이터.
 *
 * T2.2에서 presets_get 로딩 + 썸네일 카드로 대체 예정 — 그때까지
 * resources/presets.default.json의 id/라벨과 동기를 유지한다.
 */
export interface PresetType {
  id: string;
  label: string;
  /** 카드 보조 설명 (프리셋 successCriteria 요약) */
  hint: string;
}

export const PRESET_TYPES: PresetType[] = [
  { id: "storybook", label: "동화같은", hint: "파스텔톤 일러스트" },
  { id: "flat-vector", label: "플랫 벡터", hint: "단순 도형·면 색" },
  { id: "watercolor", label: "수채화", hint: "번짐 질감" },
  { id: "3d-render", label: "3D 렌더", hint: "부드러운 입체" },
  { id: "line-art", label: "라인 아트", hint: "깔끔한 선화" },
  { id: "photo-real", label: "실사풍", hint: "사진 같은 질감" },
];

export const DEFAULT_PRESET_ID = PRESET_TYPES[0].id;

/** 크기 3옵션 (04 §4.1) — SDXL 권장 해상도 버킷. */
export const SIZE_OPTIONS: { label: string; size: [number, number] }[] = [
  { label: "정방형 1:1", size: [1024, 1024] },
  { label: "가로 3:2", size: [1216, 832] },
  { label: "세로 2:3", size: [832, 1216] },
];

export const COUNT_OPTIONS = [1, 2, 3, 4];
