import { NavLink } from "react-router-dom";
import { NAV_ITEMS } from "../app/navItems";
import { useTheme } from "../stores/theme";

/** 72px 아이콘 사이드바 (04 §3). */
export default function SidebarNav() {
  const theme = useTheme((s) => s.theme);
  const toggle = useTheme((s) => s.toggle);

  return (
    <nav
      aria-label="주요 메뉴"
      className="flex h-full w-[72px] flex-col items-center border-r border-border bg-surface py-3"
    >
      <ul className="flex flex-1 flex-col gap-1">
        {NAV_ITEMS.map((item) => (
          <li key={item.path}>
            <NavLink
              to={item.path}
              aria-label={item.label}
              title={item.label}
              className={({ isActive }) =>
                [
                  "flex h-12 w-12 flex-col items-center justify-center gap-0.5 rounded-md transition-colors",
                  "hover:bg-surface-2 hover:text-text focus:outline-none focus-visible:ring-2 focus-visible:ring-primary",
                  isActive ? "bg-surface-2 text-primary" : "text-text-sub",
                ].join(" ")
              }
            >
              <span aria-hidden className="text-lg leading-none">
                {item.glyph}
              </span>
              <span className="text-[10px] leading-none">{item.label}</span>
            </NavLink>
          </li>
        ))}
      </ul>

      <div className="mt-2 flex flex-col items-center gap-2 border-t border-border pt-3">
        <span
          role="status"
          aria-label="이 작업은 기기 안에서만 처리됩니다"
          title="이 작업은 기기 안에서만 처리됩니다"
          className="flex h-8 w-8 items-center justify-center rounded-full bg-surface-2 text-sm text-success"
        >
          ●
        </span>
        {/* 작업 큐 자리 — QueueIndicator는 학습/다운로드 태스크에서 구현 */}
        <span aria-hidden title="작업 큐" className="text-text-sub">
          ⬇
        </span>
        <button
          type="button"
          onClick={toggle}
          aria-label={theme === "dark" ? "라이트 모드로 전환" : "다크 모드로 전환"}
          title="테마 전환"
          className="flex h-8 w-8 items-center justify-center rounded-md text-text-sub transition-colors hover:bg-surface-2 hover:text-text focus:outline-none focus-visible:ring-2 focus-visible:ring-primary"
        >
          {theme === "dark" ? "☀" : "☾"}
        </button>
      </div>
    </nav>
  );
}
