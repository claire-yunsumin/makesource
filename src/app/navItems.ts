/** 사이드바 내비게이션 정의 (04 §3). 라우트와 1:1 대응. */
export interface NavItem {
  path: string;
  label: string;
  /** 아이콘 글리프 (04 §3 표기). 아이콘 컴포넌트 도입 전 플레이스홀더. */
  glyph: string;
}

export const NAV_ITEMS: NavItem[] = [
  { path: "/generate", label: "생성", glyph: "✦" },
  { path: "/gallery", label: "갤러리", glyph: "▦" },
  { path: "/styles", label: "스타일", glyph: "◐" },
  { path: "/presets", label: "프리셋", glyph: "⌘" },
  { path: "/settings", label: "설정", glyph: "⚙" },
];
