import { describe, expect, it } from "vitest";
import type { Generation } from "../../lib/tauri";
import { PAGE_SIZE, cursorOf, isLastPage, mergePages } from "./galleryPaging";

function gen(id: string, createdAt: number): Generation {
  return {
    id,
    createdAt,
    imagePath: `outputs/2026-07/${id}.png`,
    thumbPath: `outputs/2026-07/${id}.png`,
    keywordKo: "통나무집",
    promptFinal: "p",
    negative: null,
    presetId: "storybook",
    presetVersion: 1,
    styleId: null,
    seed: 42,
    steps: 28,
    cfg: 6.5,
    width: 1024,
    height: 1024,
    model: null,
    favorite: false,
  };
}

describe("cursorOf", () => {
  it("마지막 항목의 createdAt:id를 만든다 (백엔드 parse_cursor 형식)", () => {
    expect(cursorOf([gen("a", 100), gen("b", 90)])).toBe("90:b");
  });

  it("빈 목록은 null", () => {
    expect(cursorOf([])).toBeNull();
  });
});

describe("mergePages", () => {
  it("이어 붙이고 id 중복은 제거한다", () => {
    const merged = mergePages([gen("a", 100)], [gen("a", 100), gen("b", 90)]);
    expect(merged.map((g) => g.id)).toEqual(["a", "b"]);
  });

  it("빈 다음 페이지는 그대로", () => {
    const existing = [gen("a", 100)];
    expect(mergePages(existing, [])).toEqual(existing);
  });
});

describe("isLastPage", () => {
  it("PAGE_SIZE 미만이면 마지막 페이지", () => {
    expect(isLastPage([gen("a", 1)])).toBe(true);
    expect(isLastPage([])).toBe(true);
    const full = Array.from({ length: PAGE_SIZE }, (_, i) => gen(`g${i}`, i));
    expect(isLastPage(full)).toBe(false);
  });
});
