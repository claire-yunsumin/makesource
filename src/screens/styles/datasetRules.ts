/**
 * 학습 마법사 ①② 순수 헬퍼 (T6.2, 04 §4.3): 데이터셋 드롭 + 캡션 테이블.
 */
import type { CaptionItem } from "../../lib/tauri";
import { isImagePath } from "./essenceRules";

/** 부족 시 경고 배너 기준(04 §4.3 ①) — 차단은 아님, 경고만. */
export const MIN_DATASET_IMAGES = 20;

/**
 * 드롭된 경로를 기존 선택에 합친다 — 이미지 확장자만, 중복 제거.
 * 에센스와 달리 장수 상한은 없다(학습 데이터셋은 많을수록 유리).
 */
export function mergeDroppedDatasetImages(
  existing: string[],
  dropped: string[],
): { paths: string[]; warning: string | null } {
  const images = dropped.filter(isImagePath);
  const seen = new Set(existing);
  const merged = [...existing];
  for (const p of images) {
    if (!seen.has(p)) {
      seen.add(p);
      merged.push(p);
    }
  }
  const warning =
    images.length < dropped.length ? "이미지 파일(PNG/JPG/WebP)만 사용할 수 있어요." : null;
  return { paths: merged, warning };
}

/** 20장 미만이면 보여줄 경고 배너 문구 (빈 선택은 배너 없음 — 아직 시작 전). */
export function underMinimumWarning(paths: string[]): string | null {
  if (paths.length === 0 || paths.length >= MIN_DATASET_IMAGES) return null;
  return `이미지가 ${paths.length}장이에요. ${MIN_DATASET_IMAGES}장 이상을 권장해요 — 적으면 학습 품질이 떨어질 수 있어요.`;
}

/** 캡션 테이블 인라인 편집: file로 찾아 caption만 갱신. */
export function updateCaption(items: CaptionItem[], file: string, caption: string): CaptionItem[] {
  return items.map((item) => (item.file === file ? { ...item, caption } : item));
}

/**
 * 일괄 찾아바꾸기 — 모든 캡션에서 find를 replace로 치환한다(트리거 단어
 * 추가 등). find가 빈 문자열이면 아무것도 바꾸지 않는다.
 */
export function applyFindReplace(
  items: CaptionItem[],
  find: string,
  replace: string,
): CaptionItem[] {
  if (find === "") return items;
  return items.map((item) => ({ ...item, caption: item.caption.split(find).join(replace) }));
}
