import { convertFileSrc } from "@tauri-apps/api/core";
import { joinImagePath } from "../../lib/imagePath";
import type { GenSession } from "./genSession";

interface ResultGridProps {
  session: GenSession;
  /** 앱 데이터 루트 절대 경로 (로드 전 null) */
  dataRoot: string | null;
  /** 이미지 aria-label용 "키워드 · 프리셋" (04 §7) */
  altLabel: string;
}

/** 우측 결과 영역 (04 §4.1): 빈 상태 / 셀별 진행 오버레이 / 결과 2×2 그리드. */
export default function ResultGrid({ session, dataRoot, altLabel }: ResultGridProps) {
  if (session.phase === "generating") {
    const percent = Math.round(session.progress * 100);
    return (
      <div className="mx-auto w-full max-w-3xl">
        <p aria-live="polite" className="mb-3 text-sm text-text-sub">
          {session.cancelRequested
            ? "생성을 멈추는 중이에요…"
            : `이미지를 만드는 중이에요 · ${percent}%`}
        </p>
        <div className="grid grid-cols-2 gap-4">
          {Array.from({ length: session.cells }, (_, i) => (
            <div
              key={i}
              className="relative flex aspect-square items-center justify-center overflow-hidden rounded-lg bg-surface-2"
            >
              <div className="w-2/3">
                <div
                  role="progressbar"
                  aria-valuenow={percent}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  className="h-1.5 overflow-hidden rounded-sm bg-border"
                >
                  <div
                    className="ease-out-ui h-full rounded-sm bg-primary transition-all duration-150"
                    style={{ width: `${percent}%` }}
                  />
                </div>
                <p className="mt-2 text-center text-xs text-text-sub">{percent}%</p>
              </div>
            </div>
          ))}
        </div>
      </div>
    );
  }

  if (session.images.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
        <span aria-hidden className="text-4xl text-text-sub">
          ✦
        </span>
        <p className="text-base font-medium text-text">아직 만든 이미지가 없어요</p>
        <p className="max-w-xs text-sm text-text-sub">
          왼쪽에서 이미지 타입을 고르고 키워드를 입력한 뒤 ⌘↵ 또는 [생성하기]를 눌러보세요.
        </p>
      </div>
    );
  }

  return (
    <div className="mx-auto grid w-full max-w-3xl grid-cols-2 gap-4">
      {session.images.map((relPath) => {
        const abs = dataRoot ? joinImagePath(dataRoot, relPath) : null;
        return (
          <div
            key={relPath}
            className="animate-grid-enter aspect-square overflow-hidden rounded-lg bg-surface-2 shadow-card"
          >
            {abs ? (
              <img
                src={convertFileSrc(abs)}
                alt={altLabel}
                aria-label={altLabel}
                className="h-full w-full object-cover"
              />
            ) : (
              <div className="flex h-full items-center justify-center p-4 text-center text-xs text-text-sub">
                이미지를 불러오지 못했어요
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
