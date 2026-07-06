import { useCallback, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { dataDir, downloadDir } from "@tauri-apps/api/path";
import { useNavigate } from "react-router-dom";
import Toast from "../../components/Toast";
import { isAppError, type AppError } from "../../lib/appError";
import { copyText } from "../../lib/clipboard";
import { APP_DATA_DIR_NAME, joinImagePath } from "../../lib/imagePath";
import { exportImage, historyList, historyToggleFavorite, type Generation } from "../../lib/tauri";
import { useGenerateStore } from "../generate/store";
import DetailModal, { type ExportFormat } from "./DetailModal";
import { regenFormState } from "./detailMeta";
import { cursorOf, isLastPage, mergePages } from "./galleryPaging";

/**
 * 갤러리 (04 §4.2): masonry 그리드 + 커서 기반 무한 스크롤(T3.1),
 * 상세 모달(T3.2). 검색·필터는 T3.3에서 추가한다.
 */
export default function GalleryScreen() {
  const [items, setItems] = useState<Generation[]>([]);
  const [loading, setLoading] = useState(false);
  const [initialLoaded, setInitialLoaded] = useState(false);
  const [hasMore, setHasMore] = useState(true);
  const [error, setError] = useState<AppError | null>(null);
  const [dataRoot, setDataRoot] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [toast, setToast] = useState<{
    message: string;
    tone: "error" | "success";
  } | null>(null);
  const navigate = useNavigate();

  // 스크롤 감시 중 중복 요청 방지 (state 반영 전 공백 구간)
  const fetching = useRef(false);
  const sentinelRef = useRef<HTMLDivElement | null>(null);
  // loadMore가 항상 최신 목록으로 커서를 만들도록 ref로 동기화
  const itemsRef = useRef<Generation[]>([]);
  useEffect(() => {
    itemsRef.current = items;
  }, [items]);

  useEffect(() => {
    dataDir()
      .then((dir) => setDataRoot(`${dir.replace(/\/$/, "")}/${APP_DATA_DIR_NAME}`))
      .catch(() => setDataRoot(null));
  }, []);

  const loadMore = useCallback(async () => {
    if (fetching.current) return;
    fetching.current = true;
    setLoading(true);
    setError(null);
    try {
      const current = itemsRef.current;
      const cursor = cursorOf(current) ?? undefined;
      const page = await historyList(cursor ? { cursor } : undefined);
      setItems((prev) => mergePages(prev, page));
      setHasMore(!isLastPage(page));
    } catch (e) {
      setError(
        isAppError(e)
          ? e
          : { code: "E_UNKNOWN", message: "히스토리를 읽지 못했어요.", detail: String(e) },
      );
    } finally {
      fetching.current = false;
      setLoading(false);
      setInitialLoaded(true);
    }
  }, []);

  // 첫 페이지
  useEffect(() => {
    void loadMore();
  }, [loadMore]);

  // 무한 스크롤: 하단 센티널 관찰
  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!sentinel || !hasMore) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) void loadMore();
      },
      { rootMargin: "600px" },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [hasMore, loadMore]);

  // ---- 상세 모달 액션 (T3.2) ----

  const handleToggleFavorite = useCallback(async (id: string) => {
    const flip = (list: Generation[]) =>
      list.map((g) => (g.id === id ? { ...g, favorite: !g.favorite } : g));
    setItems(flip); // 낙관적
    try {
      await historyToggleFavorite(id);
    } catch (e) {
      setItems(flip); // 롤백
      setToast({
        message: isAppError(e) ? e.message : "즐겨찾기를 저장하지 못했어요.",
        tone: "error",
      });
    }
  }, []);

  const handleExport = useCallback(async (id: string, format: ExportFormat) => {
    try {
      const dir = await downloadDir();
      const path = await exportImage({ id, format, destDir: dir });
      const fileName = path.split("/").pop() ?? path;
      setToast({ message: `다운로드 폴더에 저장했어요 · ${fileName}`, tone: "success" });
    } catch (e) {
      setToast({
        message: isAppError(e) ? e.message : "이미지를 저장하지 못했어요.",
        tone: "error",
      });
    }
  }, []);

  const handleCopyMeta = useCallback(async (text: string) => {
    const ok = await copyText(text);
    setToast(
      ok
        ? { message: "메타 정보를 복사했어요.", tone: "success" }
        : { message: "복사하지 못했어요.", tone: "error" },
    );
  }, []);

  // 같은 설정으로 다시 생성: 생성 화면 폼을 채우고 이동 (04 §4.2)
  const handleRegenerate = useCallback(
    (item: Generation) => {
      useGenerateStore.setState(regenFormState(item));
      navigate("/generate");
    },
    [navigate],
  );

  const selected = selectedId !== null ? (items.find((g) => g.id === selectedId) ?? null) : null;

  if (initialLoaded && items.length === 0 && !error) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
        <span aria-hidden className="text-4xl text-text-sub">
          ▦
        </span>
        <p className="text-base font-medium text-text">아직 갤러리가 비어 있어요</p>
        <p className="max-w-xs text-sm text-text-sub">
          생성 화면에서 만든 이미지가 여기에 자동으로 모여요.
        </p>
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto p-6">
      <h1 className="mb-4 text-base font-medium text-text">갤러리</h1>

      {error && (
        <div role="alert" className="mb-4 rounded-md border border-error bg-surface-2 px-3 py-2">
          <p className="text-sm text-error">{error.message}</p>
          <button
            type="button"
            onClick={() => void loadMore()}
            className="mt-2 rounded-sm border border-border px-2 py-1 text-xs text-text-sub hover:bg-surface"
          >
            다시 시도
          </button>
        </div>
      )}

      {/* masonry: CSS 컬럼 — 항목은 세로 분할을 피해 통째로 배치 */}
      <div className="columns-2 gap-4 md:columns-3 xl:columns-4">
        {items.map((item) => {
          const abs = dataRoot ? joinImagePath(dataRoot, item.thumbPath || item.imagePath) : null;
          const label = `${item.keywordKo ?? "이미지"}${item.presetId ? ` · ${item.presetId}` : ""}`;
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => setSelectedId(item.id)}
              aria-label={`${label} 상세 보기`}
              className="ease-out-ui mb-4 block w-full break-inside-avoid overflow-hidden rounded-lg bg-surface-2 text-left shadow-card transition-opacity duration-150 hover:opacity-90"
            >
              {abs ? (
                <img
                  src={convertFileSrc(abs)}
                  alt={label}
                  loading="lazy"
                  className="w-full"
                  style={
                    item.width && item.height
                      ? { aspectRatio: `${item.width} / ${item.height}` }
                      : undefined
                  }
                />
              ) : (
                <div className="flex aspect-square items-center justify-center p-4 text-center text-xs text-text-sub">
                  이미지를 불러오지 못했어요
                </div>
              )}
              <span className="block truncate px-2 py-1.5 text-xs text-text-sub">
                {label}
                {item.favorite && (
                  <span aria-label="즐겨찾기" className="ml-1 text-error">
                    ♥
                  </span>
                )}
              </span>
            </button>
          );
        })}
      </div>

      {loading && (
        <p aria-live="polite" className="py-4 text-center text-sm text-text-sub">
          불러오는 중이에요…
        </p>
      )}
      {/* 무한 스크롤 센티널 */}
      {hasMore && !error && <div ref={sentinelRef} aria-hidden className="h-1" />}
      {initialLoaded && !hasMore && items.length > 0 && (
        <p className="py-4 text-center text-xs text-text-sub">전부 봤어요</p>
      )}

      {selected && (
        <DetailModal
          item={selected}
          dataRoot={dataRoot}
          onClose={() => setSelectedId(null)}
          onToggleFavorite={(id) => void handleToggleFavorite(id)}
          onExport={(id, format) => void handleExport(id, format)}
          onCopyMeta={(text) => void handleCopyMeta(text)}
          onRegenerate={handleRegenerate}
        />
      )}
      {toast && <Toast message={toast.message} tone={toast.tone} onClose={() => setToast(null)} />}
    </div>
  );
}
