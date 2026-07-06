import { create } from "zustand";

/**
 * 경량 i18n (T7.2). 언어는 settings.json의 language(ko/en)로 저장되고,
 * 앱 시작 시 App이 불러와 setLocale로 반영한다.
 *
 * v0.5 범위(PRD: 지역화는 v1.0): 공용 컴포넌트·내비게이션 문자열만 사전에 두고,
 * 화면별 문자열은 v1.0 전체 지역화에서 옮긴다. 사전에 없는 화면은 ko 그대로 노출.
 */

export type Locale = "ko" | "en";

/** 사전 키. 값 추가 시 ko/en 둘 다 채워야 한다 (테스트가 파리티 검사). */
const MESSAGES = {
  ko: {
    "nav.aria": "주요 메뉴",
    "nav.generate": "생성",
    "nav.gallery": "갤러리",
    "nav.styles": "스타일",
    "nav.presets": "프리셋",
    "nav.settings": "설정",
    "nav.localOnly": "이 작업은 기기 안에서만 처리됩니다",
    "nav.queue": "작업 큐",
    "nav.themeToggle": "테마 전환",
    "nav.toLight": "라이트 모드로 전환",
    "nav.toDark": "다크 모드로 전환",
  },
  en: {
    "nav.aria": "Main menu",
    "nav.generate": "Generate",
    "nav.gallery": "Gallery",
    "nav.styles": "Styles",
    "nav.presets": "Presets",
    "nav.settings": "Settings",
    "nav.localOnly": "Everything is processed on this device",
    "nav.queue": "Job queue",
    "nav.themeToggle": "Toggle theme",
    "nav.toLight": "Switch to light mode",
    "nav.toDark": "Switch to dark mode",
  },
} as const satisfies Record<Locale, Record<string, string>>;

export type MessageKey = keyof (typeof MESSAGES)["ko"];

export const LOCALES: Locale[] = ["ko", "en"];

interface LocaleState {
  locale: Locale;
  setLocale: (locale: Locale) => void;
}

export const useLocale = create<LocaleState>((set) => ({
  locale: "ko",
  setLocale: (locale) => {
    // 스크린리더 발음·맞춤법 검사 언어도 함께 (04 §7)
    document.documentElement.lang = locale;
    set({ locale });
  },
}));

export function translate(locale: Locale, key: MessageKey): string {
  return MESSAGES[locale][key] ?? MESSAGES.ko[key];
}

/** 컴포넌트용 훅: `const t = useT();` → `t("nav.generate")`. */
export function useT(): (key: MessageKey) => string {
  const locale = useLocale((s) => s.locale);
  return (key) => translate(locale, key);
}

/** 테스트·검증용으로 사전 자체도 노출. */
export const I18N_MESSAGES = MESSAGES;
