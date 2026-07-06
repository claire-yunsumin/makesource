import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  TOUR_STEPS,
  nextAvailable,
  spotlightRect,
  tooltipPosition,
  type Rect,
} from "../lib/tourGuide";
import { useOnboarding } from "../stores/onboarding";

/**
 * 온보딩 코치마크 투어 (T8.1): data-tour 앵커를 스포트라이트로 비추고
 * 툴팁으로 화면을 하나씩 훑는다. 마지막 스텝은 첫 생성 유도 CTA.
 * 스텝·위치 계산은 lib/tourGuide.ts 순수 함수가 담당.
 */
export default function OnboardingTour() {
  const active = useOnboarding((s) => s.active);
  const stepIndex = useOnboarding((s) => s.stepIndex);

  const [anchorRect, setAnchorRect] = useState<Rect | null>(null);
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number } | null>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const nextButtonRef = useRef<HTMLButtonElement>(null);

  const availability = useCallback(
    () => TOUR_STEPS.map((s) => document.querySelector(`[data-tour="${s.anchor}"]`) !== null),
    [],
  );

  // 앵커 측정 (스텝 진입·리사이즈 시). 앵커가 사라졌으면 다음 스텝으로 건너뛴다.
  const measure = useCallback(() => {
    const step = TOUR_STEPS[stepIndex];
    const el = document.querySelector(`[data-tour="${step.anchor}"]`);
    if (!el) {
      const next = nextAvailable(stepIndex, 1, availability());
      if (next === null) useOnboarding.getState().finish();
      else useOnboarding.getState().goTo(next);
      return;
    }
    el.scrollIntoView({ block: "nearest" });
    const r = el.getBoundingClientRect();
    setAnchorRect({ x: r.x, y: r.y, width: r.width, height: r.height });
  }, [stepIndex, availability]);

  useLayoutEffect(() => {
    if (!active) return;
    setTooltipPos(null); // 새 스텝은 측정 후 표시 (잔상 점프 방지)
    measure();
  }, [active, measure]);

  useEffect(() => {
    if (!active) return;
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [active, measure]);

  // 툴팁 크기를 실측한 뒤 위치 확정
  useLayoutEffect(() => {
    if (!active || anchorRect === null || tooltipRef.current === null) return;
    const box = tooltipRef.current.getBoundingClientRect();
    setTooltipPos(
      tooltipPosition(
        anchorRect,
        { width: box.width, height: box.height },
        TOUR_STEPS[stepIndex].placement,
        { width: window.innerWidth, height: window.innerHeight },
      ),
    );
  }, [active, anchorRect, stepIndex]);

  // 스텝이 바뀌면 다음 버튼에 포커스 (키보드 진행)
  useEffect(() => {
    if (active && tooltipPos !== null) nextButtonRef.current?.focus();
  }, [active, stepIndex, tooltipPos]);

  // Esc = 건너뛰기 (생성 화면의 Esc 취소 리스너는 생성 중이 아니면 no-op)
  useEffect(() => {
    if (!active) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") useOnboarding.getState().finish();
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [active]);

  if (!active || anchorRect === null) return null;

  const step = TOUR_STEPS[stepIndex];
  const spot = spotlightRect(anchorRect);
  const avail = availability();
  const prevIndex = nextAvailable(stepIndex, -1, avail);
  const nextIndex = nextAvailable(stepIndex, 1, avail);
  const isLast = nextIndex === null;

  const advance = () => {
    if (isLast) {
      useOnboarding.getState().finish();
      // 첫 생성 유도: 키워드 입력칸에 포커스
      document.getElementById("gen-keyword")?.focus();
    } else {
      useOnboarding.getState().goTo(nextIndex);
    }
  };

  return createPortal(
    <div role="dialog" aria-modal="true" aria-label="온보딩 가이드" className="fixed inset-0 z-50">
      {/* 배경 클릭 차단 (스포트라이트 그림자가 시각, 이 레이어가 이벤트 담당) */}
      <div className="absolute inset-0" onClick={() => useOnboarding.getState().finish()} />

      {/* 스포트라이트: 앵커만 밝게 뚫린 오버레이 */}
      <div
        aria-hidden
        className="pointer-events-none absolute rounded-lg ring-2 ring-primary transition-all duration-200"
        style={{
          left: spot.x,
          top: spot.y,
          width: spot.width,
          height: spot.height,
          boxShadow: "0 0 0 9999px rgb(0 0 0 / 0.55)",
        }}
      />

      {/* 툴팁 */}
      <div
        ref={tooltipRef}
        className="absolute w-80 rounded-lg border border-border bg-surface p-4 shadow-xl transition-opacity duration-150"
        style={
          tooltipPos === null
            ? { left: 0, top: 0, opacity: 0 }
            : { left: tooltipPos.x, top: tooltipPos.y, opacity: 1 }
        }
      >
        <h2 className="text-sm font-semibold text-text">{step.title}</h2>
        <p className="mt-1.5 text-sm leading-relaxed text-text-sub">{step.body}</p>

        <div className="mt-4 flex items-center justify-between">
          <button
            type="button"
            onClick={() => useOnboarding.getState().finish()}
            className="shrink-0 whitespace-nowrap rounded-sm px-2 py-1 text-xs text-text-sub hover:bg-surface-2"
          >
            건너뛰기
          </button>

          <div className="flex items-center gap-1.5" aria-hidden>
            {TOUR_STEPS.map((s, i) => (
              <span
                key={s.anchor}
                className={`h-1.5 w-1.5 rounded-full ${i === stepIndex ? "bg-primary" : "bg-border"}`}
              />
            ))}
          </div>

          <div className="flex items-center gap-1.5">
            {prevIndex !== null && (
              <button
                type="button"
                onClick={() => useOnboarding.getState().goTo(prevIndex)}
                className="shrink-0 whitespace-nowrap rounded-md border border-border px-3 py-1.5 text-xs text-text-sub hover:bg-surface-2"
              >
                이전
              </button>
            )}
            <button
              ref={nextButtonRef}
              type="button"
              onClick={advance}
              className="ease-out-ui shrink-0 whitespace-nowrap rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-white transition-colors duration-150 hover:bg-primary-hover focus:outline-none focus-visible:ring-2 focus-visible:ring-primary"
            >
              {isLast ? "✦ 첫 이미지 만들기" : "다음"}
            </button>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}
