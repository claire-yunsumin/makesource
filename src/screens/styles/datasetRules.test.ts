import { describe, expect, it } from "vitest";
import type { CaptionItem } from "../../lib/tauri";
import {
  MIN_DATASET_IMAGES,
  applyFindReplace,
  mergeDroppedDatasetImages,
  underMinimumWarning,
  updateCaption,
} from "./datasetRules";

describe("mergeDroppedDatasetImages", () => {
  it("keeps only image files, dedupes, and has no upper cap", () => {
    const many = Array.from({ length: 40 }, (_, i) => `/a/${i}.png`);
    const { paths, warning } = mergeDroppedDatasetImages([], many);
    expect(paths).toHaveLength(40);
    expect(warning).toBeNull();
  });

  it("warns when non-image files are dropped, filters them out", () => {
    const { paths, warning } = mergeDroppedDatasetImages([], ["/a/1.png", "/a/notes.txt"]);
    expect(paths).toEqual(["/a/1.png"]);
    expect(warning).not.toBeNull();
  });

  it("dedupes against existing selection", () => {
    const { paths } = mergeDroppedDatasetImages(["/a/1.png"], ["/a/1.png", "/a/2.png"]);
    expect(paths).toEqual(["/a/1.png", "/a/2.png"]);
  });
});

describe("underMinimumWarning", () => {
  it("is null before anything is dropped", () => {
    expect(underMinimumWarning([])).toBeNull();
  });

  it("warns below the minimum", () => {
    const paths = Array.from({ length: 5 }, (_, i) => `/a/${i}.png`);
    expect(underMinimumWarning(paths)).toContain("5장");
  });

  it("is null at or above the minimum", () => {
    const paths = Array.from({ length: MIN_DATASET_IMAGES }, (_, i) => `/a/${i}.png`);
    expect(underMinimumWarning(paths)).toBeNull();
  });
});

const items: CaptionItem[] = [
  { file: "a.png", caption: "cat, flat color" },
  { file: "b.png", caption: "dog" },
];

describe("updateCaption", () => {
  it("updates only the matching file's caption", () => {
    const next = updateCaption(items, "a.png", "cat, edited");
    expect(next[0].caption).toBe("cat, edited");
    expect(next[1].caption).toBe("dog");
  });
});

describe("applyFindReplace", () => {
  it("replaces across all captions (e.g. inserting a trigger word)", () => {
    const next = applyFindReplace(items, "cat", "cat mystyle");
    expect(next[0].caption).toBe("cat mystyle, flat color");
    expect(next[1].caption).toBe("dog");
  });

  it("no-ops when find is empty", () => {
    expect(applyFindReplace(items, "", "x")).toEqual(items);
  });
});
