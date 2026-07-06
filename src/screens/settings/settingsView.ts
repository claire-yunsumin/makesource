import type { ModelEntry } from "../../lib/tauri";

/** 설정 화면(04 §4.5) 표시 로직 — 모델 목록 그룹핑과 용량 표기. */

/** models/ 폴더명 → 사용자 라벨. 모르는 폴더는 이름 그대로. */
export const CATEGORY_LABELS: Record<string, string> = {
  checkpoints: "체크포인트",
  loras: "LoRA",
  ipadapter: "IP-Adapter",
  clip_vision: "CLIP Vision",
  argos: "번역 모델",
  rembg: "배경 제거 모델",
  hf: "분석 모델 캐시",
};

export function categoryLabel(category: string): string {
  return CATEGORY_LABELS[category] ?? category;
}

/** 바이트 → 사람이 읽는 용량 ("3.2GB", "512MB", "0B"). 소수점은 GB에서만 1자리. */
export function formatBytes(bytes: number): string {
  if (bytes < 0) return "0B";
  const gb = 1024 ** 3;
  const mb = 1024 ** 2;
  if (bytes >= gb) {
    const v = bytes / gb;
    return `${v >= 10 ? Math.round(v) : Math.round(v * 10) / 10}GB`;
  }
  if (bytes >= mb) return `${Math.round(bytes / mb)}MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)}KB`;
  return `${bytes}B`;
}

export interface ModelGroup {
  category: string;
  label: string;
  entries: ModelEntry[];
  totalBytes: number;
}

/** 백엔드 목록(카테고리 순서 유지)을 카테고리별 그룹으로 묶는다. */
export function groupModels(entries: ModelEntry[]): ModelGroup[] {
  const groups: ModelGroup[] = [];
  for (const entry of entries) {
    let group = groups.find((g) => g.category === entry.category);
    if (!group) {
      group = {
        category: entry.category,
        label: categoryLabel(entry.category),
        entries: [],
        totalBytes: 0,
      };
      groups.push(group);
    }
    group.entries.push(entry);
    group.totalBytes += entry.sizeBytes;
  }
  return groups;
}

/** 전체 사용량 (F-5.3). */
export function totalBytes(entries: ModelEntry[]): number {
  return entries.reduce((sum, e) => sum + e.sizeBytes, 0);
}
