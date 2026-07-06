import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { isAppError, type AppError } from "../../lib/appError";
import {
  CAPTION_PROGRESS_EVENT,
  captionDataset,
  datasetCreate,
  datasetSaveCaptions,
  type CaptionItem,
  type CaptionProgressEvent,
} from "../../lib/tauri";
import {
  applyFindReplace,
  mergeDroppedDatasetImages,
  underMinimumWarning,
  updateCaption,
} from "./datasetRules";

type WizardStep = "drop" | "captioning" | "captions" | "saved";

interface TrainingWizardProps {
  onClose: () => void;
}

/**
 * 학습 마법사 ①② (04 §4.3, T6.2): 이미지 드롭(20장+ 권장, 부족 시 경고) →
 * 자동 캡션 생성 → 캡션 테이블(인라인 편집 + 일괄 찾아바꾸기) → 저장.
 * 프로파일 선택(③)·학습 대시보드(④)는 T6.3/T6.4에서 이어진다.
 */
export default function TrainingWizard({ onClose }: TrainingWizardProps) {
  const [step, setStep] = useState<WizardStep>("drop");
  const [paths, setPaths] = useState<string[]>([]);
  const [warning, setWarning] = useState<string | null>(null);
  const [progressLog, setProgressLog] = useState<string[]>([]);
  const [datasetDir, setDatasetDir] = useState<string | null>(null);
  const [items, setItems] = useState<CaptionItem[]>([]);
  const [findText, setFindText] = useState("");
  const [replaceText, setReplaceText] = useState("");
  const [error, setError] = useState<AppError | null>(null);
  const [saving, setSaving] = useState(false);
  const stepRef = useRef(step);
  stepRef.current = step;

  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (stepRef.current !== "drop" || event.payload.type !== "drop") return;
      const dropped = event.payload.paths;
      setPaths((prev) => {
        const { paths: merged, warning: w } = mergeDroppedDatasetImages(prev, dropped);
        setWarning(w);
        return merged;
      });
    });
    return () => {
      unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<CaptionProgressEvent>(CAPTION_PROGRESS_EVENT, (e) => {
      setProgressLog((prev) => [...prev.slice(-30), e.payload.message]);
    });
    return () => {
      unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, []);

  const buildDataset = useCallback(async () => {
    setStep("captioning");
    setProgressLog([]);
    setError(null);
    try {
      const dataset = await datasetCreate(paths);
      setDatasetDir(dataset.dir);
      const captioned = await captionDataset(dataset.dir);
      setItems(captioned);
      setStep("captions");
    } catch (e) {
      setError(
        isAppError(e)
          ? e
          : { code: "E_UNKNOWN", message: "데이터셋 준비에 실패했어요.", detail: String(e) },
      );
      setStep("drop");
    }
  }, [paths]);

  const handleSave = useCallback(async () => {
    if (!datasetDir || saving) return;
    setSaving(true);
    setError(null);
    try {
      await datasetSaveCaptions(datasetDir, items);
      setStep("saved");
    } catch (e) {
      setError(
        isAppError(e)
          ? e
          : { code: "E_UNKNOWN", message: "캡션을 저장하지 못했어요.", detail: String(e) },
      );
    } finally {
      setSaving(false);
    }
  }, [datasetDir, items, saving]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && stepRef.current !== "captioning") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const stepIndex = step === "drop" ? 1 : step === "captioning" || step === "captions" ? 2 : 3;
  const dropWarning = underMinimumWarning(paths);
  const buttonClass =
    "ease-out-ui rounded-md px-4 py-2 text-sm font-medium transition-colors duration-150";

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="정밀 학습(LoRA) 데이터셋 만들기"
      className="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-6"
    >
      <div className="flex max-h-full w-full max-w-3xl flex-col gap-4 overflow-y-auto rounded-lg bg-surface p-6 shadow-card">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-medium text-text">
            정밀 학습(LoRA) 데이터셋 · {stepIndex}/4 단계
          </h2>
          <button
            type="button"
            aria-label="닫기"
            onClick={onClose}
            disabled={step === "captioning"}
            className="rounded-sm px-2 py-1 text-xs text-text-sub hover:bg-surface-2 disabled:opacity-40"
          >
            ✕
          </button>
        </div>

        {error && (
          <div role="alert" className="rounded-md border border-error bg-surface-2 px-3 py-2">
            <p className="text-sm text-error">{error.message}</p>
            {error.detail && <p className="mt-1 break-all text-xs text-text-sub">{error.detail}</p>}
          </div>
        )}

        {step === "drop" && (
          <>
            <div className="flex min-h-40 flex-col items-center justify-center gap-2 rounded-lg border-2 border-dashed border-border p-6 text-center">
              <p className="text-sm text-text">학습에 쓸 이미지를 끌어다 놓으세요</p>
              <p className="text-xs text-text-sub">PNG · JPG · WebP · 여러 장 한꺼번에 가능</p>
            </div>
            {warning && <p className="text-xs text-warn">{warning}</p>}
            {dropWarning && (
              <div role="alert" className="rounded-md border border-warn bg-surface-2 px-3 py-2">
                <p className="text-xs text-warn">{dropWarning}</p>
              </div>
            )}
            {paths.length > 0 && (
              <ul className="max-h-32 space-y-1 overflow-y-auto text-xs text-text-sub">
                {paths.map((p) => (
                  <li key={p} className="flex items-center justify-between gap-2">
                    <span className="truncate">{p.split("/").pop()}</span>
                    <button
                      type="button"
                      aria-label={`${p.split("/").pop()} 빼기`}
                      onClick={() => setPaths((prev) => prev.filter((x) => x !== p))}
                      className="shrink-0 rounded-sm px-1.5 text-text-sub hover:bg-surface-2"
                    >
                      ✕
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <button
              type="button"
              disabled={paths.length === 0}
              onClick={() => void buildDataset()}
              className={`${buttonClass} bg-primary text-white hover:bg-primary-hover disabled:cursor-not-allowed disabled:opacity-50`}
            >
              캡션 자동 생성하기 ({paths.length}장)
            </button>
          </>
        )}

        {step === "captioning" && (
          <div className="space-y-3">
            <p aria-live="polite" className="text-sm text-text">
              이미지마다 캡션을 만드는 중이에요… 첫 실행은 모델 내려받기로 오래 걸릴 수 있어요.
            </p>
            <div className="max-h-40 overflow-y-auto rounded-md bg-surface-2 p-3 font-mono text-xs text-text-sub">
              {progressLog.length === 0 ? (
                <p>준비 중…</p>
              ) : (
                progressLog.map((l, i) => <p key={i}>{l}</p>)
              )}
            </div>
          </div>
        )}

        {step === "captions" && (
          <>
            <div className="flex flex-wrap items-end gap-2">
              <label className="flex-1 space-y-1">
                <span className="text-xs text-text-sub">일괄 찾아바꾸기 — 찾기</span>
                <input
                  value={findText}
                  onChange={(e) => setFindText(e.target.value)}
                  placeholder="예: cat"
                  className="w-full rounded-md border border-border bg-surface-2 px-3 py-1.5 text-sm text-text focus:border-primary focus:outline-none"
                />
              </label>
              <label className="flex-1 space-y-1">
                <span className="text-xs text-text-sub">바꾸기 (트리거 단어 추가 등)</span>
                <input
                  value={replaceText}
                  onChange={(e) => setReplaceText(e.target.value)}
                  placeholder="예: cat mystyle"
                  className="w-full rounded-md border border-border bg-surface-2 px-3 py-1.5 text-sm text-text focus:border-primary focus:outline-none"
                />
              </label>
              <button
                type="button"
                disabled={findText === ""}
                onClick={() => setItems((prev) => applyFindReplace(prev, findText, replaceText))}
                className={`${buttonClass} border border-border text-text hover:bg-surface-2 disabled:opacity-40`}
              >
                모두 바꾸기
              </button>
            </div>

            <div className="max-h-80 space-y-2 overflow-y-auto">
              {items.map((item) => (
                <div
                  key={item.file}
                  className="flex items-center gap-2 rounded-md bg-surface-2 p-2"
                >
                  <span className="w-28 shrink-0 truncate text-xs text-text-sub">{item.file}</span>
                  <input
                    value={item.caption}
                    onChange={(e) =>
                      setItems((prev) => updateCaption(prev, item.file, e.target.value))
                    }
                    className="flex-1 rounded-md border border-border bg-surface px-2 py-1 text-xs text-text focus:border-primary focus:outline-none"
                  />
                </div>
              ))}
            </div>

            <button
              type="button"
              onClick={() => void handleSave()}
              disabled={saving}
              className={`${buttonClass} bg-primary text-white hover:bg-primary-hover disabled:opacity-50`}
            >
              {saving ? "저장 중…" : "캡션 저장"}
            </button>
          </>
        )}

        {step === "saved" && (
          <div className="space-y-3 text-center">
            <p className="text-sm text-text">데이터셋과 캡션을 저장했어요.</p>
            <p className="text-xs text-text-sub">
              프로파일 선택과 학습 시작은 다음 업데이트에서 이어져요. 저장한 데이터셋은 그대로 남아
              있어요.
            </p>
            <button
              type="button"
              onClick={onClose}
              className={`${buttonClass} border border-border text-text hover:bg-surface-2`}
            >
              닫기
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
