import { describe, expect, it } from "vitest";
import {
  TOUR_STEPS,
  nextAvailable,
  spotlightRect,
  tooltipPosition,
  type Rect,
  type Size,
} from "./tourGuide";

describe("TOUR_STEPS", () => {
  it("사이드바 환영으로 시작해 생성 버튼(첫 생성 유도)으로 끝난다", () => {
    expect(TOUR_STEPS[0].anchor).toBe("nav");
    expect(TOUR_STEPS[TOUR_STEPS.length - 1].anchor).toBe("generate");
  });

  it("앵커는 중복 없고 문구는 비어 있지 않다", () => {
    const anchors = TOUR_STEPS.map((s) => s.anchor);
    expect(new Set(anchors).size).toBe(anchors.length);
    for (const step of TOUR_STEPS) {
      expect(step.title.trim()).not.toBe("");
      expect(step.body.trim()).not.toBe("");
    }
  });

  it("서비스 핵심(스타일·타입·키워드·갤러리)을 모두 훑는다", () => {
    const anchors = new Set(TOUR_STEPS.map((s) => s.anchor));
    for (const required of ["style", "preset", "keyword", "nav-gallery"]) {
      expect(anchors.has(required)).toBe(true);
    }
  });
});

describe("spotlightRect", () => {
  it("앵커 주위로 padding만큼 넓힌다", () => {
    const rect = spotlightRect({ x: 100, y: 50, width: 200, height: 40 }, 6);
    expect(rect).toEqual({ x: 94, y: 44, width: 212, height: 52 });
  });

  it("화면 왼쪽/위 밖으로는 나가지 않는다", () => {
    const rect = spotlightRect({ x: 2, y: 3, width: 50, height: 20 }, 6);
    expect(rect.x).toBe(0);
    expect(rect.y).toBe(0);
  });
});

describe("tooltipPosition", () => {
  const viewport: Size = { width: 1280, height: 800 };
  const tooltip: Size = { width: 320, height: 160 };

  it("right: 앵커 오른쪽에 gap 띄우고 세로 중앙 정렬", () => {
    const anchor: Rect = { x: 0, y: 300, width: 72, height: 200 };
    const pos = tooltipPosition(anchor, tooltip, "right", viewport, 12);
    expect(pos.x).toBe(84); // 72 + 12
    expect(pos.y).toBe(300 + 100 - 80); // 중앙 - 툴팁 절반
  });

  it("top: 앵커 위에 gap 띄우고 가로 중앙 정렬", () => {
    const anchor: Rect = { x: 400, y: 700, width: 200, height: 48 };
    const pos = tooltipPosition(anchor, tooltip, "top", viewport, 12);
    expect(pos.y).toBe(700 - 12 - 160);
    expect(pos.x).toBe(400 + 100 - 160);
  });

  it("뷰포트 밖으로 넘치면 margin 안쪽으로 클램프한다", () => {
    // 화면 바닥에 붙은 앵커의 right 배치 → 세로로 넘침
    const bottom: Rect = { x: 0, y: 780, width: 72, height: 20 };
    const posBottom = tooltipPosition(bottom, tooltip, "right", viewport, 12, 8);
    expect(posBottom.y).toBe(800 - 160 - 8);

    // 오른쪽 끝 앵커의 right 배치 → 가로로 넘침
    const right: Rect = { x: 1200, y: 100, width: 60, height: 40 };
    const posRight = tooltipPosition(right, tooltip, "right", viewport, 12, 8);
    expect(posRight.x).toBe(1280 - 320 - 8);

    // 왼쪽 위 앵커의 top 배치 → 위·왼쪽으로 넘침
    const topLeft: Rect = { x: 0, y: 0, width: 40, height: 20 };
    const posTop = tooltipPosition(topLeft, tooltip, "top", viewport, 12, 8);
    expect(posTop.x).toBe(8);
    expect(posTop.y).toBe(8);
  });
});

describe("nextAvailable", () => {
  const available = [true, false, true, true, false];

  it("앞으로: 사용 불가 스텝을 건너뛴다", () => {
    expect(nextAvailable(0, 1, available)).toBe(2);
    expect(nextAvailable(2, 1, available)).toBe(3);
  });

  it("앞으로 더 없으면 null (투어 종료)", () => {
    expect(nextAvailable(3, 1, available)).toBeNull();
  });

  it("뒤로: 사용 불가 스텝을 건너뛴다", () => {
    expect(nextAvailable(2, -1, available)).toBe(0);
    expect(nextAvailable(3, -1, available)).toBe(2);
  });

  it("뒤로 더 없으면 null (이전 버튼 숨김)", () => {
    expect(nextAvailable(0, -1, available)).toBeNull();
  });

  it("전부 사용 불가면 항상 null", () => {
    expect(nextAvailable(0, 1, [false, false])).toBeNull();
  });
});
