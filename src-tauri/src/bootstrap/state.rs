//! 부트스트랩 상태 머신 (TAD §7).
//!
//! `check → install_python → clone_comfyui → pip_install → download_models → warmup → ready`
//! 각 단계 완료 시점을 JSON 파일로 기록해, 중단 후 재실행 시 이어서 진행한다.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// 설치 단계. 순서는 `ORDER` 배열이 단일 진실.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Check,
    InstallPython,
    CloneComfyui,
    PipInstall,
    DownloadModels,
    Warmup,
    Ready,
}

impl Step {
    pub const ORDER: [Step; 7] = [
        Step::Check,
        Step::InstallPython,
        Step::CloneComfyui,
        Step::PipInstall,
        Step::DownloadModels,
        Step::Warmup,
        Step::Ready,
    ];

    /// 전체 진행률 계산용 인덱스 (0-based).
    pub fn index(self) -> usize {
        // ORDER는 모든 variant를 포함하므로 반드시 찾는다
        Self::ORDER.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// 다음 단계. Ready에서는 None.
    pub fn next(self) -> Option<Step> {
        Self::ORDER.get(self.index() + 1).copied()
    }

    /// 이 단계 시작 시점의 전체 진행률(0.0~1.0). Ready = 1.0.
    pub fn base_progress(self) -> f64 {
        self.index() as f64 / (Self::ORDER.len() - 1) as f64
    }
}

/// 모델 프로파일 (TAD §7): standard(SDXL, ~10GB) / light(SD1.5, ~4GB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProfile {
    Standard,
    Light,
}

/// 디스크에 기록되는 재개 상태.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapState {
    /// 다음에 실행할(또는 진행 중이던) 단계
    pub step: Step,
    /// 마지막 실행에서 선택한 프로파일 (재개 시 그대로 사용)
    pub model_profile: Option<ModelProfile>,
}

impl Default for BootstrapState {
    fn default() -> Self {
        Self {
            step: Step::Check,
            model_profile: None,
        }
    }
}

impl BootstrapState {
    pub fn is_ready(&self) -> bool {
        self.step == Step::Ready
    }

    /// 상태 파일 로드. 없거나 손상 시 처음부터(default).
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// 상태 파일 저장 (부모 폴더 생성 포함).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_progress_in_tad_order() {
        // TAD §7: check → install_python → clone_comfyui → pip_install → download_models → warmup → ready
        let mut step = Step::Check;
        let mut visited = vec![step];
        while let Some(next) = step.next() {
            step = next;
            visited.push(step);
        }
        assert_eq!(visited, Step::ORDER.to_vec());
        assert_eq!(step, Step::Ready);
        assert_eq!(Step::Ready.next(), None);
    }

    #[test]
    fn base_progress_is_monotonic_and_bounded() {
        let mut prev = -1.0;
        for step in Step::ORDER {
            let p = step.base_progress();
            assert!(p > prev, "{step:?} 진행률이 단조증가해야 함");
            prev = p;
        }
        assert_eq!(Step::Check.base_progress(), 0.0);
        assert_eq!(Step::Ready.base_progress(), 1.0);
    }

    #[test]
    fn step_serializes_snake_case() {
        // 이벤트 페이로드/상태 파일 계약: snake_case 문자열
        assert_eq!(
            serde_json::to_value(Step::InstallPython).unwrap(),
            "install_python"
        );
        assert_eq!(
            serde_json::to_value(Step::DownloadModels).unwrap(),
            "download_models"
        );
    }

    #[test]
    fn state_roundtrip_via_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/bootstrap.state.json");

        let state = BootstrapState {
            step: Step::PipInstall,
            model_profile: Some(ModelProfile::Light),
        };
        state.save(&path).unwrap();

        // 재실행 시나리오: 저장된 지점부터 재개
        let loaded = BootstrapState::load(&path);
        assert_eq!(loaded, state);
        assert!(!loaded.is_ready());
    }

    #[test]
    fn missing_or_corrupt_state_restarts_from_check() {
        let dir = tempfile::tempdir().unwrap();

        // 파일 없음 → 처음부터
        let missing = BootstrapState::load(&dir.path().join("none.json"));
        assert_eq!(missing.step, Step::Check);

        // 손상 → 처음부터 (패닉 없이)
        let corrupt_path = dir.path().join("corrupt.json");
        std::fs::write(&corrupt_path, "{not json").unwrap();
        assert_eq!(BootstrapState::load(&corrupt_path).step, Step::Check);
    }
}
