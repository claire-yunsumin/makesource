import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { startDrag } from "@crabnebula/tauri-plugin-drag";
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
import { buildHistoryArgs, cursorOf, isLastPage, mergePages } from "./galleryPaging";

/**
 * 갤러리 카드 1장 (T9.9, docs/11 §P6.3–4). memo로 ♥ 토글 등 목록 갱신 시
 * 변경된 항목만 다시 그린다. content-visibility로 화면 밖 카드의 렌더 비용을
 * 건너뛰고, contain-intrinsic-size로 스크롤바 점프를 막는다.
 */
const GalleryItem = memo(function GalleryItem({
  item,
  dataRoot,
  onSelect,
  onDragStart,
}: {
  item: Generation;
  dataRoot: string | null;
  onSelect: (id: string) => void;
  onDragStart: (e: React.DragEvent, item: Generation) => void;
}) {
  const abs = useMemo(
    () => (dataRoot ? joinImagePath(dataRoot, item.thumbPath || item.imagePath) : null),
    [dataRoot, item.thumbPath, item.imagePath],
  );
  const src = useMemo(() => (abs ? convertFileSrc(abs) : null), [abs]);
  const label = `${item.keywordKo ?? "이미지"}${item.presetId ? ` · ${item.presetId}` : ""}`;
  const ratio = item.width && item.height ? item.width / item.height : 1;
  return (
    <button
      type="button"
      draggable
      onDragStart={(e) => onDragStart(e, item)}
      onClick={() => onSelect(item.id)}
      aria-label={`${label} 상세 보기`}
      className="ease-out-ui mb-4 block w-full cursor-grab break-inside-avoid overflow-hidden rounded-lg bg-surface-2 text-left shadow-card transition-opacity duration-150 hover:opacity-90"
      style={{
        contentVisibility: "auto",
        containIntrinsicSize: `auto 240px auto ${Math.round(240 / ratio) + 30}px`,
      }}
    >
      {src ? (
        <img
          src={src}
          alt={label}
          loading="lazy"
          decoding="async"
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
});

/**
 * 갤러리 (04 §4.2): masonry 그리드 + 커서 기반 무한 스크롤(T3.1),
 * 상세 모달(T3.2), 검색·♥ 필터(T3.3). 스타일 필터 칩은 스타일 목록(M4)과 함께.
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
  // 검색·필터 (T3.3) — query는 300ms 디바운스 후 요청에 반영
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [favoriteOnly, setFavoriteOnly] = useState(false);
  const navigate = useNavigate();

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(query.trim()), 300);
    return () => clearTimeout(timer);
  }, [query]);

  // 스크롤 감시 중 중복 요청 방지 (state 반영 전 공백 구간)
  const fetching = useRef(false);
  const sentinelRef = useRef<HTMLDivElement | null>(null);
  // loadMore가 항상 최신 목록으로 커서를 만들도록 ref로 동기화
  const itemsRef = useRef<Generation[]>([]);
  useEffect(() => {
    itemsRef.current = items;
  }, [items]);
  // 필터가 바뀐 뒤 도착한 이전 조건의 응답을 무시하기 위한 세대 번호
  const filtersRef = useRef({ query: "", favoriteOnly: false });
  const requestSeq = useRef(0);

  useEffect(() => {
    dataDir()
      .then((dir) => setDataRoot(`${dir.replace(/\/$/, "")}/${APP_DATA_DIR_NAME}`))
      .catch(() => setDataRoot(null));
  }, []);

  const loadMore = useCallback(async () => {
    if (fetching.current) return;
    fetching.current = true;
    const seq = requestSeq.current;
    setLoading(true);
    setError(null);
    try {
      const filters = filtersRef.current;
      const page = await historyList(
        buildHistoryArgs(cursorOf(itemsRef.current), filters.query, filters.favoriteOnly),
      );
      if (seq !== requestSeq.current) return; // 필터가 바뀐 뒤 도착 — 버림
      setItems((prev) => mergePages(prev, page));
      setHasMore(!isLastPage(page));
    } catch (e) {
      if (seq !== requestSeq.current) return;
      setError(
        isAppError(e)
          ? e
          : { code: "E_UNKNOWN", message: "히스토리를 읽지 못했어요.", detail: String(e) },
      );
    } finally {
      fetching.current = false;
      if (seq === requestSeq.current) {
        setLoading(false);
        setInitialLoaded(true);
      } else {
        // 이 요청이 버려졌다면 최신 조건으로 다시 (딱 한 번 재귀)
        void loadMore();
      }
    }
  }, []);

  // 첫 페이지 + 필터 변경 시 리셋 후 재조회 (T3.3)
  useEffect(() => {
    filtersRef.current = { query: debouncedQuery, favoriteOnly };
    requestSeq.current += 1;
    itemsRef.current = [];
    setItems([]);
    setHasMore(true);
    setInitialLoaded(false);
    void loadMore();
  }, [debouncedQuery, favoriteOnly, loadMore]);

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
        message: isAppError(e) ? e.message : "즐겨찾기를 저장하지 못했어요. 다시 시도해 주세요.",
        tone: "error",
      });
    }
  }, []);

  const handleExport = useCallback(
    async (id: string, format: ExportFormat, transparent = false) => {
      try {
        if (transparent) {
          setToast({
            message: "배경을 지우는 중이에요… 최대 몇십 초 걸릴 수 있어요.",
            tone: "success",
          });
        }
        const dir = await downloadDir();
        const path = await exportImage({ id, format, transparent, destDir: dir });
        const fileName = path.split("/").pop() ?? path;
        setToast({ message: `다운로드 폴더에 저장했어요 · ${fileName}`, tone: "success" });
      } catch (e) {
        setToast({
          message: isAppError(e) ? e.message : "이미지를 저장하지 못했어요. 다시 시도해 주세요.",
          tone: "error",
        });
      }
    },
    [],
  );

  const handleCopyMeta = useCallback(async (text: string) => {
    const ok = await copyText(text);
    setToast(
      ok
        ? { message: "메타 정보를 복사했어요.", tone: "success" }
        : { message: "복사하지 못했어요. 다시 시도해 주세요.", tone: "error" },
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

  // Finder/타 앱으로 드래그 아웃 (T3.4) — HTML5 드래그를 취소하고 네이티브 드래그로
  const handleDragStart = useCallback(
    (e: React.DragEvent, item: Generation) => {
      e.preventDefault();
      const abs = dataRoot ? joinImagePath(dataRoot, item.imagePath) : null;
      if (!abs) return;
      startDrag({ item: [abs], icon: abs }).catch(() => {
        setToast({ message: "드래그를 시작하지 못했어요. 다시 시도해 주세요.", tone: "error" });
      });
    },
    [dataRoot],
  );

  const selected = selectedId !== null ? (items.find((g) => g.id === selectedId) ?? null) : null;
  const hasFilters = debouncedQuery !== "" || favoriteOnly;

  // 필터 없이도 비어 있으면 온보딩형 빈 상태 (검색 바 불필요)
  if (initialLoaded && items.length === 0 && !error && !hasFilters) {
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
      {/* 상단: 검색 + 필터 칩 (04 §4.2, T3.3) */}
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <h1 className="text-base font-medium text-text">갤러리</h1>
        <input
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="키워드 검색"
          aria-label="키워드 검색"
          className="w-full max-w-xs rounded-md border border-border bg-surface-2 px-3 py-1.5 text-sm text-text placeholder:text-text-sub focus:border-primary focus:outline-none"
        />
        <button
          type="button"
          aria-pressed={favoriteOnly}
          onClick={() => setFavoriteOnly((v) => !v)}
          className={`ease-out-ui rounded-md border px-3 py-1.5 text-xs transition-colors duration-150 ${
            favoriteOnly
              ? "border-error text-error"
              : "border-border text-text-sub hover:bg-surface-2"
          }`}
        >
          ♥ 즐겨찾기만
        </button>
      </div>

      {initialLoaded && items.length === 0 && !error && hasFilters && (
        <p className="py-10 text-center text-sm text-text-sub">조건에 맞는 이미지가 없어요.</p>
      )}

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
        {items.map((item) => (
          <GalleryItem
            key={item.id}
            item={item}
            dataRoot={dataRoot}
            onSelect={setSelectedId}
            onDragStart={handleDragStart}
          />
        ))}
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
          onExport={(id, format, transparent) => void handleExport(id, format, transparent)}
          onCopyMeta={(text) => void handleCopyMeta(text)}
          onRegenerate={handleRegenerate}
        />
      )}
      {toast && <Toast message={toast.message} tone={toast.tone} onClose={() => setToast(null)} />}
    </div>
  );
}
