/**
 * 갤러리 무한 스크롤 페이징 순수 헬퍼 (T3.1).
 * 커서 형식은 백엔드 parse_cursor와 동기: "{createdAt}:{id}"
 */
import type { Generation } from "../../lib/tauri";

/** 백엔드 history_list 페이지 크기 (commands/history.rs PAGE_SIZE와 동기). */
export const PAGE_SIZE = 40;

/** 다음 페이지 요청에 쓸 커서. 목록이 비면 null (더 요청할 수 없음). */
export function cursorOf(items: Generation[]): string | null {
  const last = items[items.length - 1];
  return last ? `${last.createdAt}:${last.id}` : null;
}

/** 페이지 병합 — id 중복 제거(늦게 온 중복은 무시). */
export function mergePages(existing: Generation[], next: Generation[]): Generation[] {
  const seen = new Set(existing.map((g) => g.id));
  return [...existing, ...next.filter((g) => !seen.has(g.id))];
}

/** 한 페이지가 PAGE_SIZE 미만이면 마지막 페이지다. */
export function isLastPage(page: Generation[]): boolean {
  return page.length < PAGE_SIZE;
}
