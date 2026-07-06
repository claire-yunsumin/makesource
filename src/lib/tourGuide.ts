/**
 * 온보딩 코치마크 투어 순수 로직 (T8.1).
 * 스텝 정의·툴팁 위치 계산·스텝 이동만 담당하고, DOM 측정·렌더링은
 * components/OnboardingTour.tsx가 한다.
 */

export type TourPlacement = "right" | "left" | "top" | "bottom";

export interface TourStep {
  /** 화면 요소의 data-tour 앵커 값 */
  anchor: string;
  title: string;
  body: string;
  placement: TourPlacement;
}

/**
 * 투어 순서 (04 §4.1 생성 화면 기준): 사이드바 → 좌패널 위→아래 → 갤러리 → 생성 버튼.
 * 마지막 스텝은 첫 생성 유도 CTA(OnboardingTour가 버튼 문구를 바꾼다).
 */
export const TOUR_STEPS: TourStep[] = [
  {
    anchor: "nav",
    title: "LocalBrush에 오신 걸 환영해요",
    body: "왼쪽 메뉴로 모든 화면을 오가요. 이미지를 만들고, 모아보고, 우리 브랜드 톤을 만드는 일 전부 — 인터넷 없이 내 Mac 안에서만 처리돼요.",
    placement: "right",
  },
  {
    anchor: "style",
    title: "스타일",
    body: "브랜드 느낌을 입히는 곳이에요. 스타일 화면에서 참고 이미지 몇 장으로 만들 수 있어요. 아직 없어도 괜찮아요 — 지금 바로 생성할 수 있어요.",
    placement: "right",
  },
  {
    anchor: "preset",
    title: "이미지 타입",
    body: "배너, 상품 컷처럼 용도를 고르면 알맞은 설정이 자동으로 적용돼요. 프리셋 화면에서 직접 다듬을 수도 있어요.",
    placement: "right",
  },
  {
    anchor: "keyword",
    title: "키워드",
    body: "만들고 싶은 걸 한글로 편하게 적으면 돼요. 내부에서 알아서 영어로 바꿔 써요.",
    placement: "right",
  },
  {
    anchor: "nav-gallery",
    title: "갤러리",
    body: "만든 이미지는 전부 여기에 쌓여요. 검색하고, 즐겨찾고, 원하는 형식으로 내보낼 수 있어요.",
    placement: "right",
  },
  {
    anchor: "generate",
    title: "이제 만들어볼까요?",
    body: "키워드 하나면 충분해요. 버튼을 누르거나 ⌘↵로 첫 이미지를 만들어보세요.",
    placement: "right",
  },
];

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Size {
  width: number;
  height: number;
}

/** 스포트라이트 박스: 앵커 주위로 padding만큼 여유 (화면 밖으로는 나가지 않게 0 클램프). */
export function spotlightRect(anchor: Rect, padding = 6): Rect {
  return {
    x: Math.max(0, anchor.x - padding),
    y: Math.max(0, anchor.y - padding),
    width: anchor.width + padding * 2,
    height: anchor.height + padding * 2,
  };
}

/**
 * 툴팁 좌상단 좌표. 앵커 기준 placement 방향으로 gap만큼 띄우고,
 * 뷰포트 밖으로 나가면 margin 안쪽으로 클램프한다.
 */
export function tooltipPosition(
  anchor: Rect,
  tooltip: Size,
  placement: TourPlacement,
  viewport: Size,
  gap = 12,
  margin = 8,
): { x: number; y: number } {
  let x: number;
  let y: number;
  switch (placement) {
    case "right":
      x = anchor.x + anchor.width + gap;
      y = anchor.y + anchor.height / 2 - tooltip.height / 2;
      break;
    case "left":
      x = anchor.x - gap - tooltip.width;
      y = anchor.y + anchor.height / 2 - tooltip.height / 2;
      break;
    case "top":
      x = anchor.x + anchor.width / 2 - tooltip.width / 2;
      y = anchor.y - gap - tooltip.height;
      break;
    case "bottom":
      x = anchor.x + anchor.width / 2 - tooltip.width / 2;
      y = anchor.y + anchor.height + gap;
      break;
  }
  x = Math.min(Math.max(x, margin), viewport.width - tooltip.width - margin);
  y = Math.min(Math.max(y, margin), viewport.height - tooltip.height - margin);
  return { x, y };
}

/**
 * from에서 dir 방향으로 가장 가까운 사용 가능한 스텝 인덱스.
 * 없으면 null (앞으로 없음 = 투어 종료, 뒤로 없음 = 이전 버튼 숨김).
 * 앵커가 화면에 없는 스텝(예: 프리셋 로딩 실패)을 건너뛰는 데 쓴다.
 */
export function nextAvailable(from: number, dir: 1 | -1, available: boolean[]): number | null {
  for (let i = from + dir; i >= 0 && i < available.length; i += dir) {
    if (available[i]) return i;
  }
  return null;
}
