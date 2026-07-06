import { useCallback, useEffect } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { joinImagePath } from "../../lib/imagePath";
import type { Generation } from "../../lib/tauri";
import { metaText } from "./detailMeta";

export type ExportFormat = "png" | "jpg" | "webp";

interface DetailModalProps {
  item: Generation;
  dataRoot: string | null;
  onClose: () => void;
  onToggleFavorite: (id: string) => void;
  onExport: (id: string, format: ExportFormat) => void;
  onCopyMeta: (text: string) => void;
  onRegenerate: (item: Generation) => void;
}

/** 갤러리 상세 모달 (04 §4.2, T3.2): 원본 미리보기 + 메타 + 액션. Esc로 닫기. */
export default function DetailModal({
  item,
  dataRoot,
  onClose,
  onToggleFavorite,
  onExport,
  onCopyMeta,
  onRegenerate,
}: DetailModalProps) {
  const abs = dataRoot ? joinImagePath(dataRoot, item.imagePath) : null;
  const title = item.keywordKo ?? "이미지";

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    },
    [onClose],
  );
  useEffect(() => {
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  const actionClass =
    "ease-out-ui rounded-md border border-border px-3 py-1.5 text-xs text-text transition-colors duration-150 hover:bg-surface-2";

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={`${title} 상세`}
      className="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-6"
      onClick={onClose}
    >
      <div
        className="flex max-h-full w-full max-w-4xl gap-0 overflow-hidden rounded-lg bg-surface shadow-card"
        onClick={(e) => e.stopPropagation()}
      >
        {/* 원본 미리보기 */}
        <div className="flex min-w-0 flex-1 items-center justify-center bg-surface-2">
          {abs ? (
            <img
              src={convertFileSrc(abs)}
              alt={title}
              className="max-h-[80vh] w-full object-contain"
            />
          ) : (
            <p className="p-8 text-sm text-text-sub">이미지를 불러오지 못했어요</p>
          )}
        </div>

        {/* 메타 + 액션 */}
        <aside className="flex w-72 shrink-0 flex-col gap-4 overflow-y-auto p-5">
          <div className="flex items-start justify-between gap-2">
            <h2 className="min-w-0 break-all text-sm font-medium text-text">{title}</h2>
            <button
              type="button"
              aria-label="닫기 (Esc)"
              onClick={onClose}
              className="ease-out-ui rounded-sm px-2 py-1 text-xs text-text-sub transition-colors duration-150 hover:bg-surface-2"
            >
              ✕
            </button>
          </div>

          <dl className="space-y-2 text-xs">
            <div>
              <dt className="text-text-sub">프롬프트</dt>
              <dd className="mt-0.5 break-all rounded-sm bg-surface-2 px-2 py-1.5 text-text">
                {item.promptFinal}
              </dd>
            </div>
            {item.negative && (
              <div>
                <dt className="text-text-sub">네거티브</dt>
                <dd className="mt-0.5 break-all text-text">{item.negative}</dd>
              </div>
            )}
            <div className="flex flex-wrap gap-x-4 gap-y-1 text-text">
              <span>시드 {item.seed}</span>
              {item.steps !== null && <span>steps {item.steps}</span>}
              {item.cfg !== null && <span>cfg {item.cfg}</span>}
              {item.width !== null && item.height !== null && (
                <span>
                  {item.width}×{item.height}
                </span>
              )}
            </div>
            {item.presetId && (
              <p className="text-text-sub">
                프리셋 {item.presetId} v{item.presetVersion ?? 1}
              </p>
            )}
          </dl>

          <button type="button" onClick={() => onCopyMeta(metaText(item))} className={actionClass}>
            메타 전체 복사
          </button>

          <div className="mt-auto space-y-2">
            <button
              type="button"
              aria-pressed={item.favorite}
              onClick={() => onToggleFavorite(item.id)}
              className={`${actionClass} w-full ${item.favorite ? "border-error text-error" : ""}`}
            >
              {item.favorite ? "♥ 즐겨찾기 해제" : "♡ 즐겨찾기"}
            </button>
            <button
              type="button"
              onClick={() => onRegenerate(item)}
              className={`${actionClass} w-full`}
            >
              같은 설정으로 다시 생성
            </button>
            <div>
              <p className="mb-1 text-xs text-text-sub">내보내기</p>
              <div className="flex gap-1">
                {(["png", "jpg", "webp"] as const).map((format) => (
                  <button
                    key={format}
                    type="button"
                    onClick={() => onExport(item.id, format)}
                    className={`${actionClass} flex-1 uppercase`}
                  >
                    {format}
                  </button>
                ))}
              </div>
            </div>
          </div>
        </aside>
      </div>
    </div>
  );
}
