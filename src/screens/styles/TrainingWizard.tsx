import { useCallback, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { isAppError, type AppError } from "../../lib/appError";
import {
  CAPTION_PROGRESS_EVENT,
  KOHYA_INSTALL_PROGRESS_EVENT,
  TRAIN_DONE_EVENT,
  TRAIN_ERROR_EVENT,
  TRAIN_PROGRESS_EVENT,
  TRAIN_SAMPLE_EVENT,
  captionDataset,
  datasetCreate,
  datasetSaveCaptions,
  kohyaInstallRun,
  kohyaInstallStatus,
  styleSave,
  trainingCancel,
  trainingStart,
  type CaptionItem,
  type CaptionProgressEvent,
  type KohyaInstallProgressEvent,
  type TrainDoneEvent,
  type TrainErrorEvent,
  type TrainProgressEvent,
  type TrainSampleEvent,
  type TrainingProfile,
} from "../../lib/tauri";
import {
  applyFindReplace,
  mergeDroppedDatasetImages,
  underMinimumWarning,
  updateCaption,
} from "./datasetRules";
import { PROFILE_OPTIONS, canStartTraining, deriveTriggerWord, formatEta } from "./trainingFlow";

type WizardStep = "drop" | "captioning" | "captions" | "profile" | "training" | "done";

/** TAD §3.3 기본 LoRA 강도. */
const DEFAULT_LORA_WEIGHT = 0.8;

interface TrainingWizardProps {
  onClose: () => void;
  /** 학습 완료 → 스타일 자동 등록 후 목록 새로고침용 */
  onSaved: () => void;
}

/**
 * 학습 마법사 4단계 (04 §4.3, T6.2/T6.4): ① 이미지 드롭(20장+ 권장) →
 * ② 캡션 테이블(인라인 편집 + 일괄 찾아바꾸기) → ③ 이름·트리거·프로파일
 * 선택(kohya 미설치면 지연 설치) → ④ 학습 대시보드(진행률·ETA·epoch 샘플
 * 스트립·취소). 완료 시 스타일(kind=lora)을 자동 등록한다.
 */
export default function TrainingWizard({ onClose, onSaved }: TrainingWizardProps) {
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
  // ③ 프로파일 단계
  const [name, setName] = useState("");
  const [triggerWord, setTriggerWord] = useState("");
  const [triggerEdited, setTriggerEdited] = useState(false);
  const [profile, setProfile] = useState<TrainingProfile>("fast");
  const [kohyaInstalled, setKohyaInstalled] = useState<boolean | null>(null);
  const [installing, setInstalling] = useState(false);
  // ④ 대시보드
  const [jobId, setJobId] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [etaSeconds, setEtaSeconds] = useState<number | undefined>(undefined);
  const [epoch, setEpoch] = useState<[number, number] | null>(null);
  const [samples, setSamples] = useState<string[]>([]);
  const [cancelRequested, setCancelRequested] = useState(false);
  const stepRef = useRef(step);
  stepRef.current = step;
  const jobIdRef = useRef<string | null>(null);
  jobIdRef.current = jobId;

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
      setStep("profile");
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

  // ③ 진입 시 kohya 설치 여부 확인 (지연 설치 — T6.1)
  useEffect(() => {
    if (step !== "profile" || kohyaInstalled !== null) return;
    kohyaInstallStatus()
      .then((s) => setKohyaInstalled(s.installed))
      .catch(() => setKohyaInstalled(false));
  }, [step, kohyaInstalled]);

  useEffect(() => {
    const unlisten = listen<KohyaInstallProgressEvent>(KOHYA_INSTALL_PROGRESS_EVENT, (e) => {
      if (!e.payload.done) return;
      setInstalling(false);
      if (e.payload.error) {
        setError(e.payload.error);
      } else {
        setKohyaInstalled(true);
      }
    });
    return () => {
      unlisten.then((fn) => fn()).catch(() => undefined);
    };
  }, []);

  const handleInstall = useCallback(async () => {
    setInstalling(true);
    setError(null);
    try {
      await kohyaInstallRun();
    } catch (e) {
      setInstalling(false);
      setError(
        isAppError(e)
          ? e
          : {
              code: "E_UNKNOWN",
              message: "학습 도구 설치를 시작하지 못했어요.",
              detail: String(e),
            },
      );
    }
  }, []);

  // 완료 시 스타일 자동 등록에 쓸 최신 값 (이벤트 리스너에서 참조)
  const nameRef = useRef(name);
  nameRef.current = name;
  const savedRef = useRef(false);

  // ④ train:// 이벤트 구독 — jobId 필터링
  useEffect(() => {
    if (!jobId) return;
    const unlistens = [
      listen<TrainProgressEvent>(TRAIN_PROGRESS_EVENT, (e) => {
        if (e.payload.jobId !== jobIdRef.current) return;
        setProgress(e.payload.progress);
        setEtaSeconds(e.payload.etaSeconds);
        if (e.payload.epoch) setEpoch(e.payload.epoch);
      }),
      listen<TrainSampleEvent>(TRAIN_SAMPLE_EVENT, (e) => {
        if (e.payload.jobId !== jobIdRef.current) return;
        setSamples((prev) =>
          prev.includes(e.payload.imagePath) ? prev : [...prev, e.payload.imagePath],
        );
      }),
      listen<TrainDoneEvent>(TRAIN_DONE_EVENT, (e) => {
        if (e.payload.jobId !== jobIdRef.current || savedRef.current) return;
        savedRef.current = true;
        // 완료 → 스타일 자동 등록 (TAD §8 5, T6.4)
        void styleSave({
          id: e.payload.styleId,
          name: nameRef.current.trim(),
          kind: "lora",
          referenceImages: [],
          loraPath: e.payload.loraPath,
          loraWeight: DEFAULT_LORA_WEIGHT,
          triggerWord: e.payload.triggerWord,
          createdAt: Date.now(),
        })
          .then(() => {
            setStep("done");
            onSaved();
          })
          .catch((err: unknown) => {
            savedRef.current = false;
            setError(
              isAppError(err)
                ? err
                : { code: "E_UNKNOWN", message: "스타일 등록에 실패했어요.", detail: String(err) },
            );
            setStep("profile");
          });
      }),
      listen<TrainErrorEvent>(TRAIN_ERROR_EVENT, (e) => {
        if (e.payload.jobId !== jobIdRef.current) return;
        setJobId(null);
        setCancelRequested(false);
        if (e.payload.error.code !== "E_CANCELED") {
          setError(e.payload.error);
        }
        setStep("profile");
      }),
    ];
    return () => {
      unlistens.forEach((u) => u.then((fn) => fn()).catch(() => undefined));
    };
  }, [jobId, onSaved]);

  const handleStart = useCallback(async () => {
    if (!datasetDir || !canStartTraining(name, triggerWord)) return;
    setError(null);
    setProgress(0);
    setEtaSeconds(undefined);
    setEpoch(null);
    setSamples([]);
    setCancelRequested(false);
    try {
      const id = await trainingStart({
        styleId: crypto.randomUUID(),
        datasetDir,
        profile,
        triggerWord: triggerWord.trim(),
      });
      setJobId(id);
      setStep("training");
    } catch (e) {
      setError(
        isAppError(e)
          ? e
          : { code: "E_UNKNOWN", message: "학습을 시작하지 못했어요.", detail: String(e) },
      );
    }
  }, [datasetDir, name, triggerWord, profile]);

  const handleCancel = useCallback(async () => {
    if (!jobId || cancelRequested) return;
    setCancelRequested(true);
    try {
      await trainingCancel(jobId);
    } catch {
      setCancelRequested(false);
    }
  }, [jobId, cancelRequested]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      // 캡션 생성·학습 중에는 Esc로 닫지 않음 (학습은 [취소] 버튼으로)
      if (e.key === "Escape" && stepRef.current !== "captioning" && stepRef.current !== "training")
        onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const stepIndex =
    step === "drop"
      ? 1
      : step === "captioning" || step === "captions"
        ? 2
        : step === "profile"
          ? 3
          : 4;
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
          <h2 className="text-sm font-medium text-text">정밀 학습(LoRA) · {stepIndex}/4 단계</h2>
          <button
            type="button"
            aria-label="닫기"
            onClick={onClose}
            disabled={step === "captioning" || step === "training"}
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

        {step === "profile" && (
          <>
            <section className="grid grid-cols-2 gap-3">
              <label className="space-y-1">
                <span className="text-xs text-text-sub">스타일 이름</span>
                <input
                  value={name}
                  placeholder="예: 우리 브랜드 톤"
                  onChange={(e) => {
                    setName(e.target.value);
                    if (!triggerEdited) setTriggerWord(deriveTriggerWord(e.target.value));
                  }}
                  className="w-full rounded-md border border-border bg-surface-2 px-3 py-2 text-sm text-text placeholder:text-text-sub focus:border-primary focus:outline-none"
                />
              </label>
              <label className="space-y-1">
                <span className="text-xs text-text-sub">트리거 단어 (영문·숫자)</span>
                <input
                  value={triggerWord}
                  placeholder="예: brandtone"
                  onChange={(e) => {
                    setTriggerEdited(true);
                    setTriggerWord(deriveTriggerWord(e.target.value));
                  }}
                  className="w-full rounded-md border border-border bg-surface-2 px-3 py-2 text-sm text-text placeholder:text-text-sub focus:border-primary focus:outline-none"
                />
              </label>
            </section>

            <section>
              <p className="mb-2 text-xs text-text-sub">프로파일 — 시간과 품질의 균형을 골라요</p>
              <div className="grid grid-cols-3 gap-2">
                {PROFILE_OPTIONS.map((option) => (
                  <button
                    key={option.id}
                    type="button"
                    onClick={() => setProfile(option.id)}
                    aria-pressed={profile === option.id}
                    className={`ease-out-ui rounded-lg border p-3 text-left transition-colors duration-150 ${
                      profile === option.id
                        ? "border-primary bg-surface-2"
                        : "border-border hover:bg-surface-2"
                    }`}
                  >
                    <p className="text-sm font-medium text-text">{option.label}</p>
                    <p className="mt-0.5 text-xs text-text-sub">{option.estimate}</p>
                    <p className="mt-1 text-xs text-text-sub">{option.description}</p>
                  </button>
                ))}
              </div>
            </section>

            {kohyaInstalled === false ? (
              <div className="space-y-2 rounded-md border border-border bg-surface-2 p-3">
                <p className="text-xs text-text">
                  처음 한 번, 학습 도구(kohya)를 설치해야 해요. 몇 분 걸릴 수 있어요.
                </p>
                <button
                  type="button"
                  disabled={installing}
                  onClick={() => void handleInstall()}
                  className={`${buttonClass} bg-primary text-white hover:bg-primary-hover disabled:opacity-50`}
                >
                  {installing ? "설치 중이에요…" : "학습 도구 설치"}
                </button>
              </div>
            ) : (
              <button
                type="button"
                disabled={kohyaInstalled !== true || !canStartTraining(name, triggerWord)}
                onClick={() => void handleStart()}
                className={`${buttonClass} bg-primary text-white hover:bg-primary-hover disabled:cursor-not-allowed disabled:opacity-50`}
              >
                학습 시작
              </button>
            )}
          </>
        )}

        {step === "training" && (
          <div className="space-y-4">
            <p aria-live="polite" className="text-sm text-text">
              {cancelRequested
                ? "학습을 멈추는 중이에요…"
                : `'${name.trim()}' 스타일을 학습하는 중이에요 · ${Math.round(progress * 100)}%`}
            </p>
            <div
              role="progressbar"
              aria-valuenow={Math.round(progress * 100)}
              aria-valuemin={0}
              aria-valuemax={100}
              className="h-2 overflow-hidden rounded-sm bg-border"
            >
              <div
                className="ease-out-ui h-full rounded-sm bg-primary transition-all duration-300"
                style={{ width: `${Math.round(progress * 100)}%` }}
              />
            </div>
            <p className="text-xs text-text-sub">
              {[
                epoch ? `epoch ${epoch[0]}/${epoch[1]}` : null,
                formatEta(etaSeconds),
                "창을 닫아도 학습은 계속돼요",
              ]
                .filter(Boolean)
                .join(" · ")}
            </p>

            {samples.length > 0 && (
              <section>
                <p className="mb-1 text-xs text-text-sub">epoch 샘플 — 톤이 잡혀가는 과정</p>
                <div className="flex gap-2 overflow-x-auto">
                  {samples.map((path) => (
                    <img
                      key={path}
                      src={convertFileSrc(path)}
                      alt="학습 중간 샘플"
                      className="h-24 w-24 shrink-0 rounded-md object-cover"
                    />
                  ))}
                </div>
              </section>
            )}

            <button
              type="button"
              disabled={cancelRequested}
              onClick={() => void handleCancel()}
              className={`${buttonClass} border border-border text-text hover:border-error hover:text-error disabled:opacity-50`}
            >
              학습 취소
            </button>
          </div>
        )}

        {step === "done" && (
          <div className="space-y-3 text-center">
            <p className="text-sm text-text">'{name.trim()}' 스타일이 준비됐어요!</p>
            <p className="text-xs text-text-sub">
              스타일 목록과 생성 화면의 스타일 선택에 나타나요. 샘플 생성으로 톤을 확인해 보세요.
            </p>
            <button
              type="button"
              onClick={onClose}
              className={`${buttonClass} bg-primary text-white hover:bg-primary-hover`}
            >
              닫기
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
