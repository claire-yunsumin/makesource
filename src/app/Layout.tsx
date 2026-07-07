import { Suspense, useEffect } from "react";
import { Outlet } from "react-router-dom";
import OnboardingTour from "../components/OnboardingTour";
import SidebarNav from "../components/SidebarNav";
import { hasSeenTour, useOnboarding } from "../stores/onboarding";

/** lazy 화면 청크 로딩 중 표시 (T9.9) — 사이드바는 유지된 채 콘텐츠 영역만 */
function ScreenFallback() {
  return (
    <div className="flex h-full items-center justify-center p-8">
      <p aria-live="polite" className="text-sm text-text-sub">
        불러오는 중이에요…
      </p>
    </div>
  );
}

/** 사이드바 + 콘텐츠 골격 (04 §3). 최소 창 1024×700은 tauri.conf.json에서 강제. */
export default function Layout() {
  // 최초 실행(부트스트랩 통과 후)이면 온보딩 투어 자동 시작 (T8.1)
  useEffect(() => {
    if (!hasSeenTour()) useOnboarding.getState().start();
  }, []);

  return (
    <div className="flex h-screen bg-bg text-text">
      <SidebarNav />
      <main className="flex-1 overflow-auto">
        <Suspense fallback={<ScreenFallback />}>
          <Outlet />
        </Suspense>
      </main>
      <OnboardingTour />
    </div>
  );
}
