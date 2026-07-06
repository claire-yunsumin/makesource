import { useEffect, useState, type ReactNode } from "react";
import { bootstrapStatus, type BootstrapStatus } from "../lib/tauri";
import BootstrapScreen from "../screens/bootstrap/BootstrapScreen";

/**
 * 최초 실행 게이트 (T7.0): 부트스트랩이 ready가 아니면 앱 본편 대신
 * 풀스크린 설치 화면(04 §4.6)을 보여준다.
 */
export default function BootstrapGate({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<BootstrapStatus | null>(null);
  const [passed, setPassed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    bootstrapStatus()
      .then((s) => {
        if (cancelled) return;
        setStatus(s);
        if (s.ready) setPassed(true);
      })
      .catch(() => {
        // 상태 조회 실패(예: 브라우저 미리보기)는 게이트를 막지 않는다 —
        // 설치가 정말 안 된 상태라면 이후 generate 등에서 엔진 에러로 드러난다
        if (!cancelled) setPassed(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (passed) return <>{children}</>;
  if (status === null) return null; // 상태 확인 중 (순간이라 스피너 없이 빈 화면)

  return (
    <div className="relative">
      <BootstrapScreen status={status} onReady={() => setPassed(true)} />
      {import.meta.env.DEV && (
        <button
          type="button"
          onClick={() => setPassed(true)}
          className="absolute bottom-4 right-4 rounded-md px-3 py-1.5 text-xs text-text-sub hover:bg-surface-2"
        >
          건너뛰기 (개발용)
        </button>
      )}
    </div>
  );
}
