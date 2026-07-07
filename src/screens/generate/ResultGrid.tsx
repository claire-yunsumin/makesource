import { memo } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { joinImagePath } from "../../lib/imagePath";
import type { GenSession } from "./genSession";

interface ResultGridProps {
  session: GenSession;
  /** 앱 데이터 루트 절대 경로 (로드 전 null) */
  dataRoot: string | null;
  /** 이미지 aria-label용 "키워드 · 프리셋" (04 §7) */
  altLabel: string;
  /** ♥ 토글 (T2.4) */
  onToggleFavorite: (id: string) => void;
  /** PNG 다운로드 (T2.4) */
  onDownload: (id: string) => void;
  /** 시드 고정 — 고급 패널 시드 입력에 배치 시드를 채운다 (T2.4, F-1.5) */
  onUseSeed: () => void;
}

/**
 * 우측 결과 영역 (04 §4.1): 빈 상태 / 셀별 진행 오버레이 / 결과 2×2 그리드 + hover 액션.
 * memo (T9.9, docs/11 §P6.5): 좌패널 폼 상태(장수·크기·시드 등) 변경이 결과
 * 그리드 재렌더로 번지지 않게 한다.
 */
function ResultGrid({
  session,
  dataRoot,
  altLabel,
  onToggleFavorite,
  onDownload,
  onUseSeed,
}: ResultGridProps) {
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

  const actionButtonClass =
    "ease-out-ui rounded-md bg-surface/90 px-2.5 py-1.5 text-xs text-text shadow-card transition-colors duration-150 hover:bg-surface-2 focus-visible:opacity-100";

  return (
    <div className="mx-auto grid w-full max-w-3xl grid-cols-2 gap-4">
      {session.images.map((image) => {
        const abs = dataRoot ? joinImagePath(dataRoot, image.path) : null;
        return (
          <div
            key={image.id || image.path}
            className="animate-grid-enter group relative aspect-square overflow-hidden rounded-lg bg-surface-2 shadow-card"
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

            {/* hover/포커스 액션 (04 §4.1: [♥][⬇ PNG][시드 재생성]) */}
            <div className="absolute inset-x-0 bottom-0 flex items-center justify-between gap-1 p-2 opacity-0 transition-opacity duration-150 focus-within:opacity-100 group-hover:opacity-100">
              <button
                type="button"
                aria-pressed={image.favorite}
                aria-label={image.favorite ? "즐겨찾기 해제" : "즐겨찾기"}
                onClick={() => onToggleFavorite(image.id)}
                className={`${actionButtonClass} ${image.favorite ? "text-error" : ""}`}
              >
                {image.favorite ? "♥" : "♡"}
              </button>
              <div className="flex gap-1">
                <button
                  type="button"
                  onClick={() => onDownload(image.id)}
                  className={actionButtonClass}
                >
                  ⬇ PNG
                </button>
                <button
                  type="button"
                  title={session.seed !== null ? `시드 ${session.seed}` : undefined}
                  onClick={onUseSeed}
                  className={actionButtonClass}
                >
                  시드 고정
                </button>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}

export default memo(ResultGrid);
