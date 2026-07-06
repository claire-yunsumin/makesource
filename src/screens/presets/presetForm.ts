/**
 * 프리셋 편집 폼 순수 로직 (04 §4.4).
 *
 * 편집 폼은 Preset의 편집 가능 필드만 다룬다(id/version/history는 저장 시
 * 서버가 재계산). 히스토리 스냅샷 복원도 "폼 값을 스냅샷으로 덮어쓰기"로
 * 표현해 presetsSave 하나로 처리한다(T5.1).
 */
import type { Preset, PresetSnapshot } from "../../lib/tauri";

export interface PresetFormValues {
  labelKo: string;
  labelEn: string;
  successCriteria: string;
  prefix: string;
  suffix: string;
  negative: string;
  steps: number;
  cfg: number;
  width: number;
  height: number;
}

/** 프리셋(또는 히스토리 스냅샷)의 편집 가능 필드를 폼 값으로. */
export function toFormValues(source: Preset | PresetSnapshot): PresetFormValues {
  return {
    labelKo: source.label?.ko ?? "",
    labelEn: source.label?.en ?? "",
    successCriteria: source.successCriteria,
    prefix: source.prefix,
    suffix: source.suffix,
    negative: source.negative,
    steps: source.params.steps,
    cfg: source.params.cfg,
    width: source.params.width,
    height: source.params.height,
  };
}

/**
 * 저장용 Preset 페이로드 조립. id는 유지, version/history는 서버가 다시
 * 계산하므로 현재 값을 그대로 실어 보내되 무시됨을 가정한다.
 */
export function toSavePayload(base: Preset, form: PresetFormValues): Preset {
  return {
    ...base,
    label: { ko: form.labelKo, en: form.labelEn },
    successCriteria: form.successCriteria,
    prefix: form.prefix,
    suffix: form.suffix,
    negative: form.negative,
    params: { steps: form.steps, cfg: form.cfg, width: form.width, height: form.height },
  };
}

/** 저장 가능 여부: prefix/negative는 프롬프트 조립(TAD §4)에 필수. */
export function isFormValid(form: PresetFormValues): boolean {
  return form.prefix.trim().length > 0 && form.negative.trim().length > 0 && form.steps > 0;
}
