import { describe, expect, it } from "vitest";
import { I18N_MESSAGES, LOCALES, translate } from "./i18n";

describe("i18n 사전", () => {
  it("ko/en 키가 완전히 일치한다 (누락 번역 방지)", () => {
    const ko = Object.keys(I18N_MESSAGES.ko).sort();
    const en = Object.keys(I18N_MESSAGES.en).sort();
    expect(en).toEqual(ko);
  });

  it("빈 문자열 번역이 없다", () => {
    for (const locale of LOCALES) {
      for (const [key, value] of Object.entries(I18N_MESSAGES[locale])) {
        expect(value.length, `${locale}:${key}`).toBeGreaterThan(0);
      }
    }
  });

  it("translate가 로케일별 값을 돌려준다", () => {
    expect(translate("ko", "nav.generate")).toBe("생성");
    expect(translate("en", "nav.generate")).toBe("Generate");
  });
});
