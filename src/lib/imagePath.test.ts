import { describe, expect, it } from "vitest";
import { joinImagePath } from "./imagePath";

const ROOT = "/Users/x/Library/Application Support/LocalBrush";

describe("joinImagePath", () => {
  it("상대 경로를 데이터 루트에 붙인다", () => {
    expect(joinImagePath(ROOT, "images/2026/07/a.png")).toBe(`${ROOT}/images/2026/07/a.png`);
  });

  it("루트 끝 슬래시와 경로 앞 ./ 를 정규화한다", () => {
    expect(joinImagePath(`${ROOT}/`, "./images/a.png")).toBe(`${ROOT}/images/a.png`);
    expect(joinImagePath(ROOT, "images//a.png")).toBe(`${ROOT}/images/a.png`);
  });

  it("루트 밖으로 나가는 경로는 거부한다", () => {
    expect(joinImagePath(ROOT, "../etc/passwd")).toBeNull();
    expect(joinImagePath(ROOT, "images/../../x.png")).toBeNull();
    expect(joinImagePath(ROOT, "/absolute/x.png")).toBeNull();
    expect(joinImagePath(ROOT, "images\\x.png")).toBeNull();
  });

  it("빈 입력은 거부한다", () => {
    expect(joinImagePath("", "a.png")).toBeNull();
    expect(joinImagePath(ROOT, "")).toBeNull();
    expect(joinImagePath(ROOT, "./")).toBeNull();
  });
});
