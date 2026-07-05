/**
 * T0.1 스캐폴딩 확인용 플레이스홀더 화면.
 * 사이드바 + 5개 라우트 골격은 T0.2에서 구성한다.
 */
export default function App() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-3 bg-bg text-text">
      <h1 className="text-2xl font-semibold">LocalBrush</h1>
      <p className="text-text-sub">로컬 AI 브랜드 그래픽 생성기</p>
      <span className="rounded-md bg-surface-2 px-3 py-1 text-sm text-text-sub">
        ● 이 작업은 기기 안에서만 처리됩니다
      </span>
    </main>
  );
}
