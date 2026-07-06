import { NavLink } from "react-router-dom";
import { NAV_ITEMS } from "../app/navItems";
import { useT } from "../lib/i18n";
import { useTheme } from "../stores/theme";

/** 72px 아이콘 사이드바 (04 §3). */
export default function SidebarNav() {
  const theme = useTheme((s) => s.theme);
  const toggle = useTheme((s) => s.toggle);
  const t = useT();

  return (
    <nav
      aria-label={t("nav.aria")}
      className="flex h-full w-[72px] flex-col items-center border-r border-border bg-surface py-3"
    >
      <ul className="flex flex-1 flex-col gap-1">
        {NAV_ITEMS.map((item) => (
          <li key={item.path}>
            <NavLink
              to={item.path}
              aria-label={t(item.labelKey)}
              title={t(item.labelKey)}
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
              <span className="text-[10px] leading-none">{t(item.labelKey)}</span>
            </NavLink>
          </li>
        ))}
      </ul>

      <div className="mt-2 flex flex-col items-center gap-2 border-t border-border pt-3">
        <span
          role="status"
          aria-label={t("nav.localOnly")}
          title={t("nav.localOnly")}
          className="flex h-8 w-8 items-center justify-center rounded-full bg-surface-2 text-sm text-success"
        >
          ●
        </span>
        {/* 작업 큐 자리 — QueueIndicator는 학습/다운로드 태스크에서 구현 */}
        <span aria-hidden title={t("nav.queue")} className="text-text-sub">
          ⬇
        </span>
        <button
          type="button"
          onClick={toggle}
          aria-label={theme === "dark" ? t("nav.toLight") : t("nav.toDark")}
          title={t("nav.themeToggle")}
          className="flex h-8 w-8 items-center justify-center rounded-md text-text-sub transition-colors hover:bg-surface-2 hover:text-text focus:outline-none focus-visible:ring-2 focus-visible:ring-primary"
        >
          {theme === "dark" ? "☀" : "☾"}
        </button>
      </div>
    </nav>
  );
}
