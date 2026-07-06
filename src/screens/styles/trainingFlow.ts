/**
 * 학습 마법사 ③④ 순수 헬퍼 (T6.4, 04 §4.3): 프로파일 선택 + 대시보드.
 */
import type { TrainingProfile } from "../../lib/tauri";

/** 04 §4.3 ③: 프로파일 카드 3종 — 라벨·예상 시간은 profiles.toml과 동기. */
export const PROFILE_OPTIONS: {
  id: TrainingProfile;
  label: string;
  estimate: string;
  description: string;
}[] = [
  { id: "fast", label: "빠름", estimate: "약 30분~1시간", description: "가볍게 톤을 잡아봐요" },
  {
    id: "standard",
    label: "표준",
    estimate: "약 1~3시간",
    description: "대부분의 브랜드에 적당해요",
  },
  {
    id: "quality",
    label: "고품질",
    estimate: "약 3~6시간",
    description: "디테일까지 꼼꼼하게 학습해요",
  },
];

/**
 * 스타일 이름 → 트리거 단어 초안. 백엔드 sanitize_trigger(영숫자·소문자,
 * 비면 "style")와 같은 규칙 — 폴더 규약과 프롬프트에 그대로 쓰인다.
 */
export function deriveTriggerWord(name: string): string {
  const cleaned = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]/g, "");
  return cleaned === "" ? "style" : cleaned;
}

/** 학습 시작 가능: 이름과 트리거 단어가 채워져야 함. */
export function canStartTraining(name: string, triggerWord: string): boolean {
  return name.trim() !== "" && triggerWord.trim() !== "";
}

/** ETA 초 → "약 N분/시간" (04 §6 톤 — 초 단위 정밀도는 불안만 줌). */
export function formatEta(etaSeconds: number | undefined): string | null {
  if (etaSeconds === undefined || etaSeconds < 0) return null;
  if (etaSeconds < 90) return "1분 남짓 남았어요";
  const minutes = Math.round(etaSeconds / 60);
  if (minutes < 60) return `약 ${minutes}분 남았어요`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0 ? `약 ${hours}시간 남았어요` : `약 ${hours}시간 ${rest}분 남았어요`;
}
