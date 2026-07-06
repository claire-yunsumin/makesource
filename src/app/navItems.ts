import type { MessageKey } from "../lib/i18n";

/** 사이드바 내비게이션 정의 (04 §3). 라우트와 1:1 대응. */
export interface NavItem {
  path: string;
  /** i18n 사전 키 (T7.2) — 표시 문자열은 useT()로 해석 */
  labelKey: MessageKey;
  /** 아이콘 글리프 (04 §3 표기). 아이콘 컴포넌트 도입 전 플레이스홀더. */
  glyph: string;
}

export const NAV_ITEMS: NavItem[] = [
  { path: "/generate", labelKey: "nav.generate", glyph: "✦" },
  { path: "/gallery", labelKey: "nav.gallery", glyph: "▦" },
  { path: "/styles", labelKey: "nav.styles", glyph: "◐" },
  { path: "/presets", labelKey: "nav.presets", glyph: "⌘" },
  { path: "/settings", labelKey: "nav.settings", glyph: "⚙" },
];
