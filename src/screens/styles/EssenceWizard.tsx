import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { isAppError, type AppError } from "../../lib/appError";
import {
  ESSENCE_PROGRESS_EVENT,
  essenceCreate,
  styleSave,
  type EssenceProgressEvent,
  type EssenceResult,
  type Style,
} from "../../lib/tauri";
import { IP_WEIGHT, MAX_IMAGES, MIN_IMAGES, canAnalyze, mergeDroppedPaths } from "./essenceRules";

type WizardStep = "drop" | "analyzing" | "edit" | "save";

interface EssenceWizardProps {
  onClose: () => void;
  /** 저장 성공 — 목록 새로고침용 */
  onSaved: () => void;
}

/**
 * 에센스 마법사 (04 §4.3, T4.2): ① 참조 이미지 3~10장 드롭 → ② 분석(진행 로그)
 * → 에센스 프롬프트 다듬기 + IP-Adapter 강도 → ③ 이름 짓고 저장.
 * 샘플 검증 생성은 IP-Adapter 경로 연결(T4.3) 후 생성 화면에서.
 */
export default function EssenceWizard({ onClose, onSaved }: EssenceWizardProps) {
  const [step, setStep] = useState<WizardStep>("drop");
  const [paths, setPaths] = useState<string[]>([]);
  const [warning, setWarning] = useState<string | null>(null);
  const [progressLog, setProgressLog] = useState<string[]>([]);
  const [result, setResult] = useState<EssenceResult | null>(null);
  const [essencePrompt, setEssencePrompt] = useState("");
  const [ipWeight, setIpWeight] = useState(IP_WEIGHT.default);
  const [name, setName] = useState("");
  const [error, setError] = useState<AppError | null>(null);
  const [saving, setSaving] = useState(false);
  const stepRef = useRef(step);
  stepRef.current = step;

  // 드롭존: 웹뷰 파일 드롭 이벤트 (경로 필요 — HTML5 드롭은 경로를 안 줌)
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (stepRef.current !== "drop" || event.payload.type !== "drop") return;
      const dropped = event.payload.paths;
      setPaths((prev) => {
        const { paths: merged, warning: w } = mergeDroppedPaths(prev, dropped);
        setWarning(w);
        return merged;
      });
    });
    return () => {
      unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, []);

  // 분석 진행 로그 (essence://progress)
  useEffect(() => {
    const unlisten = listen<EssenceProgressEvent>(ESSENCE_PROGRESS_EVENT, (e) => {
      setProgressLog((prev) => [...prev.slice(-30), e.payload.message]);
    });
    return () => {
      unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, []);

  const analyze = useCallback(async () => {
    setStep("analyzing");
    setProgressLog([]);
    setError(null);
    try {
      const res = await essenceCreate(paths);
      setResult(res);
      setEssencePrompt(res.essencePrompt);
      setStep("edit");
    } catch (e) {
      setError(
        isAppError(e)
          ? e
          : { code: "E_UNKNOWN", message: "스타일 분석에 실패했어요.", detail: String(e) },
      );
      setStep("drop");
    }
  }, [paths]);

  const save = useCallback(async () => {
    if (name.trim() === "" || essencePrompt.trim() === "" || saving) return;
    setSaving(true);
    setError(null);
    const style: Style = {
      id: crypto.randomUUID(),
      name: name.trim(),
      kind: "essence",
      essencePrompt: essencePrompt.trim(),
      referenceImages: paths, // 절대 경로 — 백엔드가 styles/{id}/로 복사 후 상대화
      ipAdapterWeight: ipWeight,
      createdAt: Date.now(),
    };
    try {
      await styleSave(style);
      onSaved();
      onClose();
    } catch (e) {
      setError(
        isAppError(e)
          ? e
          : { code: "E_UNKNOWN", message: "스타일을 저장하지 못했어요.", detail: String(e) },
      );
      setSaving(false);
    }
  }, [name, essencePrompt, paths, ipWeight, saving, onSaved, onClose]);

  // Esc 닫기 (분석 중 제외 — 백엔드 취소 지점이 없음)
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && stepRef.current !== "analyzing") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const stepIndex = step === "drop" ? 1 : step === "analyzing" || step === "edit" ? 2 : 3;
  const inputClass =
    "w-full rounded-md border border-border bg-surface-2 px-3 py-2 text-sm text-text placeholder:text-text-sub focus:border-primary focus:outline-none";
  const buttonClass =
    "ease-out-ui rounded-md px-4 py-2 text-sm font-medium transition-colors duration-150";

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="에센스 스타일 만들기"
      className="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-6"
    >
      <div className="flex max-h-full w-full max-w-xl flex-col gap-4 overflow-y-auto rounded-lg bg-surface p-6 shadow-card">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-medium text-text">
            빠른 스타일 (에센스) · {stepIndex}/3 단계
          </h2>
          <button
            type="button"
            aria-label="닫기"
            onClick={onClose}
            disabled={step === "analyzing"}
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
              <p className="text-sm text-text">
                브랜드 톤을 보여주는 이미지 {MIN_IMAGES}~{MAX_IMAGES}장을 끌어다 놓으세요
              </p>
              <p className="text-xs text-text-sub">PNG · JPG · WebP</p>
            </div>
            {warning && <p className="text-xs text-warn">{warning}</p>}
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
              disabled={!canAnalyze(paths)}
              onClick={() => void analyze()}
              className={`${buttonClass} bg-primary text-white hover:bg-primary-hover disabled:cursor-not-allowed disabled:opacity-50`}
            >
              스타일 분석하기 ({paths.length}/{MIN_IMAGES}장 이상)
            </button>
          </>
        )}

        {step === "analyzing" && (
          <div className="space-y-3">
            <p aria-live="polite" className="text-sm text-text">
              스타일을 분석하는 중이에요… 첫 실행은 모델 내려받기로 몇 분 걸릴 수 있어요.
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

        {step === "edit" && result && (
          <>
            <section>
              <label htmlFor="essence-prompt" className="mb-1 block text-xs text-text-sub">
                에센스 프롬프트 — 형태·질감·컬러·구도 서술을 다듬어 주세요
              </label>
              <textarea
                id="essence-prompt"
                rows={4}
                value={essencePrompt}
                onChange={(e) => setEssencePrompt(e.target.value)}
                className={`${inputClass} resize-none`}
              />
            </section>
            {result.tags.length > 0 && (
              <section>
                <h3 className="mb-1 text-xs text-text-sub">공통 태그</h3>
                <div className="flex flex-wrap gap-1">
                  {result.tags.slice(0, 20).map((tag) => (
                    <span
                      key={tag}
                      className="rounded-sm border border-border px-2 py-0.5 text-xs text-text-sub"
                    >
                      {tag}
                    </span>
                  ))}
                </div>
              </section>
            )}
            {result.captions.length > 0 && (
              <details>
                <summary className="cursor-pointer text-xs text-text-sub">
                  이미지 서술 보기 (참고용)
                </summary>
                <ul className="mt-2 space-y-1 text-xs text-text-sub">
                  {result.captions.map((caption, i) => (
                    <li key={i} className="break-all">
                      {caption}
                    </li>
                  ))}
                </ul>
              </details>
            )}
            <section>
              <label htmlFor="ip-weight" className="mb-1 block text-xs text-text-sub">
                참조 이미지 반영 강도 (IP-Adapter) · {ipWeight.toFixed(2)}
              </label>
              <input
                id="ip-weight"
                type="range"
                min={IP_WEIGHT.min}
                max={IP_WEIGHT.max}
                step={IP_WEIGHT.step}
                value={ipWeight}
                onChange={(e) => setIpWeight(Number(e.target.value))}
                className="w-full"
              />
            </section>
            <button
              type="button"
              onClick={() => setStep("save")}
              disabled={essencePrompt.trim() === ""}
              className={`${buttonClass} bg-primary text-white hover:bg-primary-hover disabled:opacity-50`}
            >
              다음 — 이름 짓고 저장
            </button>
          </>
        )}

        {step === "save" && (
          <>
            <section>
              <label htmlFor="style-name" className="mb-1 block text-xs text-text-sub">
                스타일 이름
              </label>
              <input
                id="style-name"
                type="text"
                value={name}
                placeholder="예: 우리 브랜드 톤"
                onChange={(e) => setName(e.target.value)}
                className={inputClass}
              />
            </section>
            <p className="text-xs text-text-sub">
              저장하면 생성 화면의 스타일 선택에 나타나요. 샘플 생성으로 톤을 확인하고, 아쉬우면
              정밀 학습(LoRA)으로 업그레이드할 수 있어요.
            </p>
            <div className="flex gap-2">
              <button
                type="button"
                onClick={() => setStep("edit")}
                className={`${buttonClass} border border-border text-text hover:bg-surface-2`}
              >
                이전
              </button>
              <button
                type="button"
                onClick={() => void save()}
                disabled={name.trim() === "" || saving}
                className={`${buttonClass} flex-1 bg-primary text-white hover:bg-primary-hover disabled:opacity-50`}
              >
                {saving ? "저장 중…" : "스타일 저장"}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
