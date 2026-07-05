/**
 * 히스토리 이미지 경로 유틸.
 *
 * DB에는 앱 데이터 루트 기준 상대 경로만 저장된다 (CLAUDE.md 주의사항).
 * 화면에 표시할 때 이 함수로 절대 경로를 조합한 뒤 convertFileSrc(asset protocol)에 넘긴다.
 */

/** 앱 데이터 루트 폴더명. src-tauri/src/paths.rs의 APP_DATA_DIR_NAME과 동기. */
export const APP_DATA_DIR_NAME = "LocalBrush";

/**
 * 데이터 루트 + 상대 경로 → 절대 경로. 루트 밖으로 나가는 경로(`..`, 절대 경로,
 * 백슬래시)는 표시하지 않도록 null을 반환한다.
 */
export function joinImagePath(dataRoot: string, relPath: string): string | null {
  if (!dataRoot || !relPath) return null;
  if (relPath.startsWith("/") || relPath.includes("\\")) return null;

  const parts = relPath.split("/").filter((p) => p !== "" && p !== ".");
  if (parts.length === 0 || parts.some((p) => p === "..")) return null;

  const root = dataRoot.endsWith("/") ? dataRoot.slice(0, -1) : dataRoot;
  return `${root}/${parts.join("/")}`;
}
