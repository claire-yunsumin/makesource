/**
 * A/B 비교(04 §4.4)는 좌우 두 프리셋을 반드시 같은 시드로 생성해야 의미가
 * 있다 — 시드 값 자체를 두 generate() 호출 전에 한 번만 확정한다.
 */
import { parseSeed } from "../generate/genSession";

/**
 * @param seedInput 고급 입력 원문(빈칸 = 랜덤)
 * @param fallbackSeed 빈칸일 때 한 번만 호출해 두 쪽에 공유할 시드를 뽑는다
 * @returns 확정된 시드, 또는 입력이 유효하지 않으면 null
 */
export function resolveComparisonSeed(
  seedInput: string,
  fallbackSeed: () => number,
): number | null {
  const parsed = parseSeed(seedInput);
  if (parsed === null) return null;
  return parsed ?? fallbackSeed();
}
