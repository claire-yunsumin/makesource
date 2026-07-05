//! OOM 폴백 정책 (TAD §6, T1.5).
//!
//! ComfyUI 에러/프로세스 사망 감지 시:
//! ① 해상도 한 단계 하향 → ② SD1.5 폴백 — 순서대로 각 1회 자동 재시도.
//! 결정 로직은 순수 함수로 두고 유닛 테스트한다 (AC: 폴백 경로 검증).

use crate::error::AppError;

/// 지원 해상도 사다리 (내림차순). "한 단계 하향" = 현재보다 작은 첫 항목.
pub const RES_LADDER: [u32; 5] = [1536, 1280, 1024, 768, 512];

/// 한 변 기준 한 단계 하향. 512 이하면 더 내릴 수 없음.
pub fn downgrade_dim(dim: u32) -> Option<u32> {
    RES_LADDER.iter().copied().find(|&step| step < dim)
}

/// 두 변 모두 한 단계 하향. 어느 한 변도 못 내리면 None.
pub fn downgrade_resolution(width: u32, height: u32) -> Option<(u32, u32)> {
    match (downgrade_dim(width), downgrade_dim(height)) {
        (Some(w), Some(h)) => Some((w, h)),
        _ => None,
    }
}

/// OOM(또는 프로세스 사망) 의심 에러인지 판정.
/// - E_ENGINE_WS: 실행 중 연결 끊김 = 프로세스 사망 의심 (TAD §6)
/// - E_ENGINE_EXEC: 메시지에 OOM 관련 패턴 포함 시
pub fn is_oom_error(err: &AppError) -> bool {
    if err.code == "E_ENGINE_WS" {
        return true;
    }
    if err.code != "E_ENGINE_EXEC" {
        return false;
    }
    let haystack =
        format!("{} {}", err.message, err.detail.as_deref().unwrap_or("")).to_lowercase();
    [
        "out of memory",
        "oom",
        "mps backend",
        "invalid buffer size",
        "cannot allocate",
    ]
    .iter()
    .any(|pat| haystack.contains(pat))
}

/// 재시도 1회분의 파라미터.
#[derive(Debug, Clone, PartialEq)]
pub struct Attempt {
    pub width: u32,
    pub height: u32,
    pub checkpoint: Option<String>,
    /// 0=원본, 1=해상도 하향, 2=SD1.5 폴백
    pub stage: u8,
}

/// 실패한 시도에서 다음 시도를 결정한다. 반환: (다음 시도, 사용자 고지 문구 — 04 §6 톤).
/// - OOM성 에러가 아니면 None (즉시 실패 전파)
/// - stage 0 → 해상도 하향 (불가하면 SD1.5로 건너뜀)
/// - stage 1 → SD1.5 폴백 (이미 SD1.5거나 없으면 None)
/// - stage 2 → None (재시도 소진)
pub fn next_attempt(
    current: &Attempt,
    err: &AppError,
    sd15_checkpoint: Option<&str>,
) -> Option<(Attempt, String)> {
    if !is_oom_error(err) {
        return None;
    }

    let sd15_available =
        sd15_checkpoint.filter(|sd15| current.checkpoint.as_deref() != Some(*sd15));

    if current.stage == 0 {
        if let Some((w, h)) = downgrade_resolution(current.width, current.height) {
            return Some((
                Attempt {
                    width: w,
                    height: h,
                    checkpoint: current.checkpoint.clone(),
                    stage: 1,
                },
                format!("메모리가 부족해 크기를 낮춰 다시 시도했어요. ({w}×{h})"),
            ));
        }
        // 더 못 내리면 SD1.5 단계로 건너뜀 (아래 stage 1 처리로 폴스루)
    }

    if current.stage <= 1 {
        if let Some(sd15) = sd15_available {
            return Some((
                Attempt {
                    width: current.width,
                    height: current.height,
                    checkpoint: Some(sd15.to_string()),
                    stage: 2,
                },
                "메모리가 부족해 가벼운 모델(SD1.5)로 다시 시도했어요.".to_string(),
            ));
        }
    }

    None
}

/// models/checkpoints에서 SD1.5 계열 체크포인트 탐색 (폴백 대상).
pub fn resolve_sd15(data_root: &std::path::Path) -> Option<String> {
    let dir = data_root.join("models/checkpoints");
    std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|n| n.ends_with(".safetensors") && (n.starts_with("v1-5") || n.contains("sd15")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oom_exec_error() -> AppError {
        AppError::with_detail(
            "E_ENGINE_EXEC",
            "이미지를 만들다 문제가 생겼어요.",
            "MPS backend out of memory (MPS allocated: 8.00 GB)",
        )
    }

    fn base_attempt() -> Attempt {
        Attempt {
            width: 1024,
            height: 1024,
            checkpoint: Some("sd_xl_base_1.0.safetensors".into()),
            stage: 0,
        }
    }

    #[test]
    fn detects_oom_variants_and_process_death() {
        assert!(is_oom_error(&oom_exec_error()));
        assert!(is_oom_error(&AppError::with_detail(
            "E_ENGINE_EXEC",
            "m",
            "CUDA out of memory"
        )));
        assert!(is_oom_error(&AppError::with_detail(
            "E_ENGINE_EXEC",
            "m",
            "Invalid buffer size: 24.00 GB"
        )));
        // 프로세스 사망(WS 끊김)도 OOM 의심 (TAD §6)
        assert!(is_oom_error(&AppError::new("E_ENGINE_WS", "끊김")));
        // OOM 아닌 실행 에러/기타 에러는 폴백 안 함
        assert!(!is_oom_error(&AppError::with_detail(
            "E_ENGINE_EXEC",
            "m",
            "shape mismatch"
        )));
        assert!(!is_oom_error(&AppError::new("E_PRESET_NOT_FOUND", "없음")));
    }

    #[test]
    fn resolution_ladder_steps_down_once() {
        assert_eq!(downgrade_resolution(1024, 1024), Some((768, 768)));
        assert_eq!(downgrade_resolution(768, 768), Some((512, 512)));
        assert_eq!(downgrade_resolution(2048, 2048), Some((1536, 1536)));
        // 최저 해상도에서는 더 못 내림
        assert_eq!(downgrade_resolution(512, 512), None);
    }

    // ---- AC: 폴백 경로 (① 해상도 하향 → ② SD1.5 → 소진) ----

    #[test]
    fn full_fallback_path_resolution_then_sd15_then_give_up() {
        let sd15 = Some("v1-5-pruned-emaonly-fp16.safetensors");

        // ① 원본 실패 → 해상도 하향
        let (a1, notice1) = next_attempt(&base_attempt(), &oom_exec_error(), sd15).unwrap();
        assert_eq!((a1.width, a1.height), (768, 768));
        assert_eq!(a1.checkpoint.as_deref(), Some("sd_xl_base_1.0.safetensors"));
        assert!(notice1.contains("크기를 낮춰"));

        // ② 하향도 실패 → SD1.5 폴백
        let (a2, notice2) = next_attempt(&a1, &oom_exec_error(), sd15).unwrap();
        assert_eq!(a2.checkpoint.as_deref(), sd15);
        assert_eq!((a2.width, a2.height), (768, 768)); // 하향된 해상도 유지
        assert!(notice2.contains("SD1.5"));

        // ③ 그것도 실패 → 소진
        assert!(next_attempt(&a2, &oom_exec_error(), sd15).is_none());
    }

    #[test]
    fn non_oom_error_does_not_trigger_fallback() {
        let err = AppError::with_detail("E_ENGINE_EXEC", "m", "shape mismatch");
        assert!(next_attempt(&base_attempt(), &err, Some("v1-5.safetensors")).is_none());
    }

    #[test]
    fn min_resolution_skips_to_sd15() {
        let cur = Attempt {
            width: 512,
            height: 512,
            ..base_attempt()
        };
        let (next, notice) =
            next_attempt(&cur, &oom_exec_error(), Some("v1-5.safetensors")).unwrap();
        assert_eq!(next.checkpoint.as_deref(), Some("v1-5.safetensors"));
        assert!(notice.contains("SD1.5"));
    }

    #[test]
    fn already_on_sd15_cannot_fallback_to_itself() {
        // light 프로파일: 처음부터 SD1.5 → ② 단계는 없음
        let cur = Attempt {
            width: 512,
            height: 512,
            checkpoint: Some("v1-5-pruned-emaonly-fp16.safetensors".into()),
            stage: 0,
        };
        assert!(next_attempt(
            &cur,
            &oom_exec_error(),
            Some("v1-5-pruned-emaonly-fp16.safetensors")
        )
        .is_none());
    }

    #[test]
    fn resolve_sd15_finds_v15_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let ckpt = dir.path().join("models/checkpoints");
        std::fs::create_dir_all(&ckpt).unwrap();
        assert_eq!(resolve_sd15(dir.path()), None);
        std::fs::write(ckpt.join("sd_xl_base_1.0.safetensors"), b"x").unwrap();
        assert_eq!(resolve_sd15(dir.path()), None); // SDXL은 폴백 대상 아님
        std::fs::write(ckpt.join("v1-5-pruned-emaonly-fp16.safetensors"), b"x").unwrap();
        assert_eq!(
            resolve_sd15(dir.path()),
            Some("v1-5-pruned-emaonly-fp16.safetensors".into())
        );
    }
}
