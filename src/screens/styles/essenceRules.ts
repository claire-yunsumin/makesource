/**
 * 에센스 마법사 순수 헬퍼 (T4.2, 04 §4.3).
 */

export const MIN_IMAGES = 3;
export const MAX_IMAGES = 10;

const IMAGE_EXTS = ["png", "jpg", "jpeg", "webp"];

export function isImagePath(path: string): boolean {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return IMAGE_EXTS.includes(ext);
}

/**
 * 드롭된 경로를 기존 선택에 합친다 — 이미지 확장자만, 중복 제거, 최대 MAX_IMAGES.
 * 이미지가 아닌 파일이 섞여 있으면 warning을 함께 돌려준다.
 */
export function mergeDroppedPaths(
  existing: string[],
  dropped: string[],
): { paths: string[]; warning: string | null } {
  const images = dropped.filter(isImagePath);
  const seen = new Set(existing);
  const merged = [...existing];
  for (const p of images) {
    if (!seen.has(p) && merged.length < MAX_IMAGES) {
      seen.add(p);
      merged.push(p);
    }
  }
  let warning: string | null = null;
  if (images.length < dropped.length) {
    warning = "이미지 파일(PNG/JPG/WebP)만 사용할 수 있어요.";
  } else if (existing.length + images.length > MAX_IMAGES) {
    warning = `최대 ${MAX_IMAGES}장까지만 사용해요.`;
  }
  return { paths: merged, warning };
}

/** 분석 시작 가능 여부 (3~10장). */
export function canAnalyze(paths: string[]): boolean {
  return paths.length >= MIN_IMAGES && paths.length <= MAX_IMAGES;
}

/** IP-Adapter 강도 슬라이더 범위 (04 §4.3). */
export const IP_WEIGHT = { min: 0, max: 1, step: 0.05, default: 0.6 };
