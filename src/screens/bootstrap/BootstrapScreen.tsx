import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  BOOTSTRAP_PROGRESS_EVENT,
  bootstrapOpenLog,
  bootstrapRun,
  type BootstrapProgressEvent,
  type BootstrapStatus,
  type ModelProfile,
} from "../../lib/tauri";
import {
  isResume,
  PROFILE_CARDS,
  progressPercent,
  UI_STEPS,
  uiStepStatuses,
  type UiStepStatus,
} from "./bootstrapView";

type Phase = "select" | "running" | "error";

interface Props {
  status: BootstrapStatus;
  /** warmup까지 끝나 앱 본편으로 넘어갈 때 */
  onReady: () => void;
}

const buttonClass =
  "rounded-md px-4 py-2 text-sm font-medium transition-colors ease-out-ui focus:outline-none focus-visible:ring-2 focus-visible:ring-primary";

function StepRow({ label, status }: { label: string; status: UiStepStatus }) {
  return (
    <li className="flex items-center gap-3">
      <span
        aria-hidden
        className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-xs ${
          status === "done"
            ? "bg-success text-white"
            : status === "active"
              ? "bg-primary text-white"
              : "border border-border text-text-sub"
        }`}
      >
        {status === "done" ? "✓" : status === "active" ? "…" : ""}
      </span>
      <span className={status === "pending" ? "text-text-sub" : "text-text"}>{label}</span>
    </li>
  );
}

/**
 * 최초 실행 풀스크린 설치 화면 (04 §4.6, T7.0).
 * 프로파일 선택 → bootstrap_run → bootstrap://progress 구독으로 단계/진행률 표시.
 * 실패 시 에러 요약 + [로그 열기] + [재시도].
 */
export default function BootstrapScreen({ status, onReady }: Props) {
  const [phase, setPhase] = useState<Phase>("select");
  const [profile, setProfile] = useState<ModelProfile>(status.suggestedProfile);
  const [step, setStep] = useState(status.step);
  const [progress, setProgress] = useState(status.progress);
  const [message, setMessage] = useState("");
  const [errorMessage, setErrorMessage] = useState("");
  // onReady가 렌더마다 바뀌어도 리스너를 다시 붙이지 않도록 ref로 고정
  const onReadyRef = useRef(onReady);
  onReadyRef.current = onReady;

  useEffect(() => {
    const unlisten = listen<BootstrapProgressEvent>(BOOTSTRAP_PROGRESS_EVENT, (e) => {
      setStep(e.payload.step);
      setProgress(e.payload.progress);
      setMessage(e.payload.message);
      if (e.payload.error) {
        setErrorMessage(e.payload.message);
        setPhase("error");
        return;
      }
      if (e.payload.step === "ready") {
        onReadyRef.current();
      }
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  async function start() {
    setErrorMessage("");
    setPhase("running");
    try {
      await bootstrapRun(profile);
    } catch (err) {
      // 이미 진행 중(E_BOOTSTRAP_RUNNING)이면 이벤트 구독으로 이어서 보면 된다
      const code = (err as { code?: string } | null)?.code;
      if (code !== "E_BOOTSTRAP_RUNNING") {
        setErrorMessage(
          (err as { message?: string } | null)?.message ?? "설치를 시작하지 못했어요.",
        );
        setPhase("error");
      }
    }
  }

  const statuses = uiStepStatuses(step);
  const percent = progressPercent(progress);

  return (
    <main className="flex min-h-screen items-center justify-center bg-bg px-6 text-text">
      <div className="w-full max-w-lg space-y-8">
        <header className="space-y-2 text-center">
          <h1 className="text-2xl font-semibold">LocalBrush 준비하기</h1>
          <p className="text-sm text-text-sub">
            내 컴퓨터에서만 동작하는 이미지 생성 환경을 설치해요. 인터넷 연결이 필요해요.
          </p>
        </header>

        <ol aria-label="설치 단계" className="space-y-3 rounded-lg bg-surface p-5 shadow-card">
          {UI_STEPS.map((s, i) => (
            <StepRow key={s.label} label={s.label} status={statuses[i]} />
          ))}
        </ol>

        {phase === "select" && (
          <section className="space-y-4">
            <fieldset className="grid grid-cols-2 gap-3">
              <legend className="mb-2 text-sm text-text-sub">모델 구성을 선택해 주세요</legend>
              {PROFILE_CARDS.map((card) => (
                <label
                  key={card.profile}
                  className={`ease-out-ui relative cursor-pointer rounded-lg border p-4 transition-colors ${
                    profile === card.profile
                      ? "border-primary bg-surface"
                      : "border-border bg-surface hover:bg-surface-2"
                  }`}
                >
                  <input
                    type="radio"
                    name="profile"
                    value={card.profile}
                    checked={profile === card.profile}
                    onChange={() => setProfile(card.profile)}
                    className="sr-only"
                  />
                  <span className="flex items-center gap-2 font-medium">
                    {card.title}
                    {status.suggestedProfile === card.profile && (
                      <span className="rounded-full bg-primary px-2 py-0.5 text-[10px] font-medium text-white">
                        추천
                      </span>
                    )}
                  </span>
                  <span className="mt-1 block text-xs text-text-sub">{card.description}</span>
                  <span className="mt-2 block text-xs font-medium text-text">{card.size}</span>
                </label>
              ))}
            </fieldset>
            <button
              type="button"
              onClick={() => void start()}
              className={`${buttonClass} w-full bg-primary text-white hover:bg-primary-hover`}
            >
              {isResume(status.step) ? "이어서 설치하기" : "설치 시작하기"}
            </button>
            {isResume(status.step) && (
              <p className="text-center text-xs text-text-sub">
                지난 설치가 중간에 멈췄어요. 완료된 단계는 건너뛰어요.
              </p>
            )}
          </section>
        )}

        {phase === "running" && (
          <section className="space-y-3">
            <div
              role="progressbar"
              aria-valuenow={percent}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label="전체 설치 진행률"
              className="h-2 overflow-hidden rounded-full bg-surface-2"
            >
              <div
                className="ease-out-ui h-full bg-primary transition-all"
                style={{ width: `${percent}%` }}
              />
            </div>
            <p aria-live="polite" className="text-center text-sm text-text-sub">
              {message || "설치를 준비하는 중이에요…"} · {percent}%
            </p>
          </section>
        )}

        {phase === "error" && (
          <section className="space-y-4">
            <div className="rounded-lg border border-error bg-surface p-4">
              <p className="text-sm font-medium text-error">설치가 멈췄어요</p>
              <p aria-live="polite" className="mt-1 text-sm text-text">
                {errorMessage}
              </p>
            </div>
            <div className="flex justify-center gap-3">
              <button
                type="button"
                onClick={() => void bootstrapOpenLog().catch(() => undefined)}
                className={`${buttonClass} border border-border text-text hover:bg-surface-2`}
              >
                로그 열기
              </button>
              <button
                type="button"
                onClick={() => void start()}
                className={`${buttonClass} bg-primary text-white hover:bg-primary-hover`}
              >
                재시도
              </button>
            </div>
          </section>
        )}
      </div>
    </main>
  );
}
