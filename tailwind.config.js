/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      // 04 §2 디자인 토큰. 실제 값은 src/index.css의 CSS 변수로 정의(다크/라이트 분기).
      colors: {
        bg: "var(--color-bg)",
        surface: "var(--color-surface)",
        "surface-2": "var(--color-surface-2)",
        border: "var(--color-border)",
        text: {
          DEFAULT: "var(--color-text)",
          sub: "var(--color-text-sub)",
        },
        primary: {
          DEFAULT: "var(--color-primary)",
          hover: "var(--color-primary-hover)",
        },
        success: "var(--color-success)",
        warn: "var(--color-warn)",
        error: "var(--color-error)",
      },
      borderRadius: {
        sm: "6px",
        md: "10px",
        lg: "16px",
      },
      boxShadow: {
        card: "0 1px 3px rgba(0, 0, 0, 0.3)",
      },
      transitionTimingFunction: {
        "ease-out-ui": "cubic-bezier(0, 0, 0.2, 1)",
      },
    },
  },
  plugins: [],
};
