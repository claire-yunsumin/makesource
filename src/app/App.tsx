import { useEffect } from "react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { useLocale } from "../lib/i18n";
import { settingsGet } from "../lib/tauri";
import Layout from "./Layout";
import BootstrapGate from "./BootstrapGate";
import GenerateScreen from "../screens/generate/GenerateScreen";
import GalleryScreen from "../screens/gallery/GalleryScreen";
import StylesScreen from "../screens/styles/StylesScreen";
import PresetsScreen from "../screens/presets/PresetsScreen";
import SettingsScreen from "../screens/settings/SettingsScreen";

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
