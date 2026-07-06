import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { dataDir } from "@tauri-apps/api/path";
import Toast from "../../components/Toast";
import { isAppError } from "../../lib/appError";
import { APP_DATA_DIR_NAME } from "../../lib/imagePath";
import {
  GEN_DONE_EVENT,
  GEN_ERROR_EVENT,
  GEN_PROGRESS_EVENT,
  generate,
  generateCancel,
  presetsGet,
  translateKeyword,
  type GenDoneEvent,
  type GenErrorEvent,
  type GenProgressEvent,
  type Translation,
} from "../../lib/tauri";
import { parseSeed } from "./genSession";
import { COUNT_OPTIONS, SIZE_OPTIONS, presetLabel } from "./presetTypes";
import { containsHangul, previewPrompt, translationSourceLabel } from "./translationPreview";
import ResultGrid from "./ResultGrid";
import { useGenerateStore } from "./store";

/** 생성 화면 (04 §4.1): 좌패널 320px + 결과 그리드, 상태 3종, ⌘↵ 생성 / Esc 취소. */
export default function GenerateScreen() {
  const presets = useGenerateStore((s) => s.presets);
  const presetsLoading = useGenerateStore((s) => s.presetsLoading);
  const presetsError = useGenerateStore((s) => s.presetsError);
  const presetId = useGenerateStore((s) => s.presetId);
  const keyword = useGenerateStore((s) => s.keyword);
  const count = useGenerateStore((s) => s.count);
  const sizeIndex = useGenerateStore((s) => s.sizeIndex);
  const seedInput = useGenerateStore((s) => s.seedInput);
  const session = useGenerateStore((s) => s.session);

  const [dataRoot, setDataRoot] = useState<string | null>(null);
  const [toastMessage, setToastMessage] = useState<string | null>(null);
  // 고급 패널의 한→영 변환 미리보기 (디바운스, T2.3)
  const [translation, setTranslation] = useState<Translation | null>(null);
  // ⌘↵ 연타로 invoke가 겹치지 않게 하는 가드 (start 반영 전 공백 구간)
  const submitting = useRef(false);

  // 이미지 표시용 데이터 루트 (src-tauri/src/paths.rs와 동일 규약)
  useEffect(() => {
    dataDir()
      .then((dir) => setDataRoot(`${dir.replace(/\/$/, "")}/${APP_DATA_DIR_NAME}`))
      .catch(() => setDataRoot(null));
  }, []);

  // 이미지 타입 프리셋 로딩 (presets_get). 실패 시 재시도 버튼으로 이 콜백 재호출.
  const loadPresets = useCallback(async () => {
    useGenerateStore.setState({ presetsLoading: true, presetsError: null });
    try {
      const list = await presetsGet();
      useGenerateStore.getState().setPresets(list);
    } catch (e) {
      useGenerateStore
        .getState()
        .setPresetsError(
          isAppError(e)
            ? e
            : { code: "E_UNKNOWN", message: "프리셋을 불러오지 못했어요.", detail: String(e) },
        );
    }
  }, []);

  useEffect(() => {
    void loadPresets();
  }, [loadPresets]);

  // 키워드 변환 미리보기 (500ms 디바운스). 한글 없으면 로컬에서 즉시 처리.
  useEffect(() => {
    const trimmed = keyword.trim();
    if (trimmed === "") {
      setTranslation(null);
      return;
    }
    if (!containsHangul(trimmed)) {
      setTranslation({ translated: trimmed, source: "notNeeded" });
      return;
    }
    let stale = false;
    const timer = setTimeout(() => {
      translateKeyword(trimmed)
        .then((t) => {
          if (!stale) setTranslation(t);
        })
        .catch(() => {
          if (!stale) {
            setTranslation({
              translated: trimmed,
              source: "passthrough",
              warning: "변환 미리보기를 불러오지 못했어요. 생성에는 영향이 없어요.",
            });
          }
        });
    }, 500);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [keyword]);

  // gen:// 이벤트 구독 — jobId 필터링은 genSession 순수 함수가 담당
  useEffect(() => {
    const unlistens = [
      listen<GenProgressEvent>(GEN_PROGRESS_EVENT, (e) =>
        useGenerateStore.getState().onProgress(e.payload),
      ),
      listen<GenDoneEvent>(GEN_DONE_EVENT, (e) => useGenerateStore.getState().onDone(e.payload)),
      listen<GenErrorEvent>(GEN_ERROR_EVENT, (e) => useGenerateStore.getState().onError(e.payload)),
    ];
    return () => {
      unlistens.forEach((p) => p.then((fn) => fn()).catch(() => undefined));
    };
  }, []);

  const submit = useCallback(async () => {
    const st = useGenerateStore.getState();
    const trimmed = st.keyword.trim();
    const seed = parseSeed(st.seedInput);
    if (st.session.phase === "generating" || submitting.current) return;
    if (trimmed === "" || seed === null) return;

    submitting.current = true;
    try {
      const jobId = await generate({
        presetId: st.presetId,
        keyword: trimmed,
        count: st.count,
        size: SIZE_OPTIONS[st.sizeIndex].size,
        seed,
      });
      st.start(jobId, st.count);
    } catch (e) {
      st.failLocal(
        isAppError(e)
          ? e
          : { code: "E_UNKNOWN", message: "생성을 시작하지 못했어요.", detail: String(e) },
      );
    } finally {
      submitting.current = false;
    }
  }, []);

  const cancel = useCallback(async () => {
    const st = useGenerateStore.getState();
    const jobId = st.session.jobId;
    if (st.session.phase !== "generating" || jobId === null || st.session.cancelRequested) return;
    st.markCancelRequested();
    try {
      await generateCancel(jobId);
    } catch {
      // 잡이 이미 끝난 경우 등 — 이어지는 gen:// 이벤트가 상태를 정리한다
    }
  }, []);

  // 단축키: ⌘↵ 생성, Esc 취소 (04 §4.1)
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        void submit();
      } else if (e.key === "Escape") {
        void cancel();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [submit, cancel]);

  // 새 에러가 오면 토스트도 함께 표시 (배너 + 토스트, 04 §4.1)
  useEffect(() => {
    if (session.error) setToastMessage(session.error.message);
  }, [session.error]);

  const generating = session.phase === "generating";
  const seedInvalid = parseSeed(seedInput) === null;
  const canSubmit = keyword.trim() !== "" && presetId !== "" && !seedInvalid && !generating;
  const selectedPreset = presets.find((p) => p.id === presetId);
  const presetLabelText = selectedPreset ? presetLabel(selectedPreset) : presetId || "이미지";

  return (
    <div className="flex h-full">
      {/* 좌측 패널 */}
      <aside className="flex w-[320px] shrink-0 flex-col gap-5 overflow-y-auto border-r border-border bg-surface p-5">
        {session.phase === "error" && session.error && (
          <div
            role="alert"
            className="rounded-md border border-error bg-surface-2 px-3 py-2 text-sm"
          >
            <p className="font-medium text-error">{session.error.message}</p>
            {session.error.detail && (
              <p className="mt-1 break-all text-xs text-text-sub">{session.error.detail}</p>
            )}
            <button
              type="button"
              onClick={() => useGenerateStore.getState().dismissError()}
              className="mt-2 rounded-sm px-2 py-1 text-xs text-text-sub hover:bg-surface"
            >
              닫기
            </button>
          </div>
        )}

        <section>
          <h2 className="mb-2 text-xs font-medium text-text-sub">스타일</h2>
          <div className="flex items-center gap-2">
            <span className="rounded-md border border-primary bg-surface-2 px-3 py-1.5 text-sm text-text">
              없음
            </span>
            <span className="text-xs text-text-sub">에센스·LoRA는 스타일 화면에서 (준비 중)</span>
          </div>
        </section>

        <section>
          <h2 className="mb-2 text-xs font-medium text-text-sub">이미지 타입</h2>
          {presetsLoading ? (
            <div className="grid grid-cols-2 gap-2" aria-hidden>
              {Array.from({ length: 6 }).map((_, i) => (
                <div
                  key={i}
                  className="h-14 animate-pulse rounded-md border border-border bg-surface-2"
                />
              ))}
            </div>
          ) : presetsError ? (
            <div className="rounded-md border border-error bg-surface-2 px-3 py-2 text-sm">
              <p className="text-error">{presetsError.message}</p>
              <button
                type="button"
                onClick={() => void loadPresets()}
                className="mt-2 rounded-sm border border-border px-2 py-1 text-xs text-text-sub hover:bg-surface"
              >
                다시 시도
              </button>
            </div>
          ) : presets.length === 0 ? (
            <p className="text-xs text-text-sub">사용할 수 있는 프리셋이 없어요.</p>
          ) : (
            <div className="grid grid-cols-2 gap-2">
              {presets.map((p) => {
                const selected = p.id === presetId;
                return (
                  <button
                    key={p.id}
                    type="button"
                    aria-pressed={selected}
                    onClick={() => useGenerateStore.getState().setPresetId(p.id)}
                    className={`ease-out-ui rounded-md border px-3 py-2 text-left transition-colors duration-150 ${
                      selected
                        ? "border-primary bg-surface-2"
                        : "border-border bg-surface hover:bg-surface-2"
                    }`}
                  >
                    <span className="block text-sm text-text">{presetLabel(p)}</span>
                    <span className="line-clamp-2 block text-xs text-text-sub">
                      {p.successCriteria}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </section>

        <section>
          <label htmlFor="gen-keyword" className="mb-2 block text-xs font-medium text-text-sub">
            키워드
          </label>
          <textarea
            id="gen-keyword"
            rows={3}
            value={keyword}
            placeholder="예: 통나무집"
            onChange={(e) => useGenerateStore.getState().setKeyword(e.target.value)}
            className="w-full resize-none rounded-md border border-border bg-surface-2 px-3 py-2 text-sm text-text placeholder:text-text-sub focus:border-primary focus:outline-none"
          />
        </section>

        <section className="flex gap-4">
          <div>
            <h2 className="mb-2 text-xs font-medium text-text-sub">장수</h2>
            <div className="flex gap-1">
              {COUNT_OPTIONS.map((n) => (
                <button
                  key={n}
                  type="button"
                  aria-pressed={n === count}
                  onClick={() => useGenerateStore.getState().setCount(n)}
                  className={`ease-out-ui h-8 w-8 rounded-sm border text-sm transition-colors duration-150 ${
                    n === count
                      ? "border-primary bg-surface-2 text-text"
                      : "border-border text-text-sub hover:bg-surface-2"
                  }`}
                >
                  {n}
                </button>
              ))}
            </div>
          </div>
          <div className="min-w-0 flex-1">
            <h2 className="mb-2 text-xs font-medium text-text-sub">크기</h2>
            <div className="flex flex-wrap gap-1">
              {SIZE_OPTIONS.map((opt, i) => (
                <button
                  key={opt.label}
                  type="button"
                  aria-pressed={i === sizeIndex}
                  onClick={() => useGenerateStore.getState().setSizeIndex(i)}
                  className={`ease-out-ui rounded-sm border px-2 py-1.5 text-xs transition-colors duration-150 ${
                    i === sizeIndex
                      ? "border-primary bg-surface-2 text-text"
                      : "border-border text-text-sub hover:bg-surface-2"
                  }`}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>
        </section>

        <details>
          <summary className="cursor-pointer text-xs font-medium text-text-sub">고급</summary>
          <div className="mt-3">
            <label htmlFor="gen-seed" className="mb-1 block text-xs text-text-sub">
              시드 (빈칸이면 매번 랜덤)
            </label>
            <input
              id="gen-seed"
              type="text"
              inputMode="numeric"
              value={seedInput}
              placeholder="랜덤"
              onChange={(e) => useGenerateStore.getState().setSeedInput(e.target.value)}
              aria-invalid={seedInvalid}
              className={`w-full rounded-md border bg-surface-2 px-3 py-2 text-sm text-text placeholder:text-text-sub focus:outline-none ${
                seedInvalid ? "border-error" : "border-border focus:border-primary"
              }`}
            />
            {seedInvalid && <p className="mt-1 text-xs text-error">정수만 입력할 수 있어요.</p>}

            <h3 className="mb-1 mt-4 text-xs text-text-sub">변환·프롬프트 미리보기</h3>
            {translation === null ? (
              <p className="text-xs text-text-sub">
                키워드를 입력하면 실제 사용될 영문 프롬프트를 보여드려요.
              </p>
            ) : (
              <div className="space-y-1">
                {translation.source !== "notNeeded" && (
                  <p className="text-xs text-text">
                    {keyword.trim()} → {translation.translated}
                    {translationSourceLabel(translation.source) && (
                      <span className="text-text-sub">
                        {" "}
                        · {translationSourceLabel(translation.source)}
                      </span>
                    )}
                  </p>
                )}
                {translation.warning && <p className="text-xs text-warn">{translation.warning}</p>}
                {selectedPreset && (
                  <p className="break-all rounded-sm bg-surface-2 px-2 py-1.5 text-xs text-text-sub">
                    {previewPrompt(selectedPreset, translation.translated)}
                  </p>
                )}
              </div>
            )}
          </div>
        </details>

        <div className="mt-auto pt-2">
          {generating ? (
            <button
              type="button"
              onClick={() => void cancel()}
              disabled={session.cancelRequested}
              className="ease-out-ui w-full rounded-md border border-border bg-surface-2 px-4 py-3 text-sm font-medium text-text transition-colors duration-150 hover:bg-surface disabled:cursor-not-allowed disabled:opacity-60"
            >
              {session.cancelRequested ? "멈추는 중…" : "취소 (Esc)"}
            </button>
          ) : (
            <button
              type="button"
              onClick={() => void submit()}
              disabled={!canSubmit}
              className="ease-out-ui w-full rounded-md bg-primary px-4 py-3 text-sm font-medium text-white transition-colors duration-150 hover:bg-primary-hover disabled:cursor-not-allowed disabled:opacity-50"
            >
              ✦ 생성하기 (⌘↵)
            </button>
          )}
        </div>
      </aside>

      {/* 우측 결과 영역 */}
      <section className="flex-1 overflow-y-auto p-8">
        {session.notice && (
          <div
            role="status"
            className="mx-auto mb-4 w-full max-w-3xl rounded-md border border-warn bg-surface-2 px-3 py-2 text-sm text-warn"
          >
            {session.notice}
          </div>
        )}
        <ResultGrid
          session={session}
          dataRoot={dataRoot}
          altLabel={`${keyword.trim() || "이미지"} · ${presetLabelText}`}
        />
      </section>

      {toastMessage && (
        <Toast message={toastMessage} tone="error" onClose={() => setToastMessage(null)} />
      )}
    </div>
  );
}
