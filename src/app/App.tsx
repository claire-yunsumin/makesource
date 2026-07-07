import { lazy, useEffect } from "react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { useLocale } from "../lib/i18n";
import { settingsGet } from "../lib/tauri";
import Layout from "./Layout";
import BootstrapGate from "./BootstrapGate";

// 화면 단위 코드 스플리팅 (T9.9, docs/11 §P6.1) — 첫 페인트에 필요한 건
// 레이아웃과 첫 라우트뿐이다. 학습/에센스 마법사가 딸린 화면을 eager로 다
// 담으면 초기 번들이 그만큼 커진다.
const GenerateScreen = lazy(() => import("../screens/generate/GenerateScreen"));
const GalleryScreen = lazy(() => import("../screens/gallery/GalleryScreen"));
const StylesScreen = lazy(() => import("../screens/styles/StylesScreen"));
const PresetsScreen = lazy(() => import("../screens/presets/PresetsScreen"));
const SettingsScreen = lazy(() => import("../screens/settings/SettingsScreen"));

/**
 * 라우팅 골격 (T0.2). 데스크톱 웹뷰에서 새로고침/딥링크 404를 피하려고 HashRouter 사용.
 * 기본 진입은 생성 화면(메인, 04 §4.1). 부트스트랩 미완료면 게이트가 설치 화면을 먼저 보여준다(T7.0).
 */
export default function App() {
  const setLocale = useLocale((s) => s.setLocale);

  // 저장된 언어 설정 반영 (T7.2). 실패(미리보기 등)는 기본 ko 유지.
  useEffect(() => {
    settingsGet()
      .then((s) => setLocale(s.language))
      .catch(() => undefined);
  }, [setLocale]);

  return (
    <BootstrapGate>
      <HashRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route index element={<Navigate to="/generate" replace />} />
            <Route path="/generate" element={<GenerateScreen />} />
            <Route path="/gallery" element={<GalleryScreen />} />
            <Route path="/styles" element={<StylesScreen />} />
            <Route path="/presets" element={<PresetsScreen />} />
            <Route path="/settings" element={<SettingsScreen />} />
            <Route path="*" element={<Navigate to="/generate" replace />} />
          </Route>
        </Routes>
      </HashRouter>
    </BootstrapGate>
  );
}
