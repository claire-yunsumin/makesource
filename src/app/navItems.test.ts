import { describe, expect, it } from "vitest";
import { I18N_MESSAGES } from "../lib/i18n";
import { NAV_ITEMS } from "./navItems";

describe("NAV_ITEMS", () => {
  it("5개 메뉴를 정의한다", () => {
    expect(NAV_ITEMS).toHaveLength(5);
  });

  it("경로가 유일하며 절대 경로다", () => {
    const paths = NAV_ITEMS.map((i) => i.path);
    expect(new Set(paths).size).toBe(paths.length);
    paths.forEach((p) => expect(p.startsWith("/")).toBe(true));
  });

  it("모든 항목의 라벨 키가 사전에 있고 글리프가 있다", () => {
    NAV_ITEMS.forEach((i) => {
      expect(I18N_MESSAGES.ko[i.labelKey]).toBeTruthy();
      expect(I18N_MESSAGES.en[i.labelKey]).toBeTruthy();
      expect(i.glyph.length).toBeGreaterThan(0);
    });
  });
});
