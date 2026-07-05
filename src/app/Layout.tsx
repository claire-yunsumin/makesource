import { Outlet } from "react-router-dom";
import SidebarNav from "../components/SidebarNav";

/** 사이드바 + 콘텐츠 골격 (04 §3). 최소 창 1024×700은 tauri.conf.json에서 강제. */
export default function Layout() {
  return (
    <div className="flex h-screen bg-bg text-text">
      <SidebarNav />
      <main className="flex-1 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
