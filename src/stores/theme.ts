import { create } from "zustand";

export type Theme = "dark" | "light";

interface ThemeState {
  theme: Theme;
  toggle: () => void;
}

/**
 * 전역 테마 상태 (다크 우선, 04 §1). 04 §2 토큰은 index.css에서 .dark 클래스로 분기하므로
 * documentElement의 클래스만 토글하면 된다. index.html은 기본 dark로 시작.
 */
export const useTheme = create<ThemeState>((set) => ({
  theme: "dark",
  toggle: () =>
    set((state) => {
      const next: Theme = state.theme === "dark" ? "light" : "dark";
      document.documentElement.classList.toggle("dark", next === "dark");
      return { theme: next };
    }),
}));
