import { describe, expect, it } from "vitest";
import { MAX_IMAGES, canAnalyze, isImagePath, mergeDroppedPaths } from "./essenceRules";

describe("isImagePath", () => {
  it("이미지 확장자만 허용 (대소문자 무시)", () => {
    expect(isImagePath("/a/b.png")).toBe(true);
    expect(isImagePath("/a/b.JPG")).toBe(true);
    expect(isImagePath("/a/b.webp")).toBe(true);
    expect(isImagePath("/a/b.pdf")).toBe(false);
    expect(isImagePath("/a/b")).toBe(false);
  });
});

describe("mergeDroppedPaths", () => {
  it("이미지만 합치고 중복은 제거한다", () => {
    const { paths, warning } = mergeDroppedPaths(["/a.png"], ["/a.png", "/b.jpg"]);
    expect(paths).toEqual(["/a.png", "/b.jpg"]);
    expect(warning).toBeNull();
  });

  it("이미지가 아닌 파일이 섞이면 경고", () => {
    const { paths, warning } = mergeDroppedPaths([], ["/a.png", "/doc.pdf"]);
    expect(paths).toEqual(["/a.png"]);
    expect(warning).toContain("이미지 파일");
  });

  it("최대 장수를 넘기면 자르고 경고", () => {
    const existing = Array.from({ length: MAX_IMAGES - 1 }, (_, i) => `/e${i}.png`);
    const { paths, warning } = mergeDroppedPaths(existing, ["/x.png", "/y.png"]);
    expect(paths.length).toBe(MAX_IMAGES);
    expect(warning).toContain("최대");
  });
});

describe("canAnalyze", () => {
  it("3~10장에서만 true", () => {
    expect(canAnalyze(["/1.png", "/2.png"])).toBe(false);
    expect(canAnalyze(["/1.png", "/2.png", "/3.png"])).toBe(true);
    expect(canAnalyze(Array.from({ length: 11 }, (_, i) => `/${i}.png`))).toBe(false);
  });
});
