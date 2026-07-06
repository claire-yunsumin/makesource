import { describe, expect, it } from "vitest";
import type { ModelEntry } from "../../lib/tauri";
import { categoryLabel, formatBytes, groupModels, totalBytes } from "./settingsView";

const GB = 1024 ** 3;
const MB = 1024 ** 2;

describe("formatBytes", () => {
  it("단위별로 사람이 읽는 표기를 만든다", () => {
    expect(formatBytes(0)).toBe("0B");
    expect(formatBytes(500)).toBe("500B");
    expect(formatBytes(2048)).toBe("2KB");
    expect(formatBytes(512 * MB)).toBe("512MB");
    expect(formatBytes(3.25 * GB)).toBe("3.3GB");
    expect(formatBytes(10.4 * GB)).toBe("10GB");
  });

  it("음수는 0B로 방어한다", () => {
    expect(formatBytes(-1)).toBe("0B");
  });
});

describe("groupModels / totalBytes", () => {
  const entries: ModelEntry[] = [
    { name: "sdxl.safetensors", category: "checkpoints", sizeBytes: 6 * GB },
    { name: "sd15.safetensors", category: "checkpoints", sizeBytes: 4 * GB },
    { name: "mystyle.safetensors", category: "loras", sizeBytes: 100 * MB },
    { name: "hf", category: "hf", sizeBytes: 1 * GB },
  ];

  it("백엔드 순서를 유지하며 카테고리로 묶고 그룹 용량을 합산한다", () => {
    const groups = groupModels(entries);
    expect(groups.map((g) => g.category)).toEqual(["checkpoints", "loras", "hf"]);
    expect(groups[0].entries).toHaveLength(2);
    expect(groups[0].totalBytes).toBe(10 * GB);
    expect(groups[0].label).toBe("체크포인트");
  });

  it("전체 사용량을 합산한다 (F-5.3)", () => {
    expect(totalBytes(entries)).toBe(10 * GB + 100 * MB + 1 * GB);
    expect(totalBytes([])).toBe(0);
  });
});

describe("categoryLabel", () => {
  it("알려진 폴더는 한국어 라벨, 모르는 폴더는 이름 그대로", () => {
    expect(categoryLabel("loras")).toBe("LoRA");
    expect(categoryLabel("unknown_dir")).toBe("unknown_dir");
  });
});
