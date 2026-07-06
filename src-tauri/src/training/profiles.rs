//! 학습 프로파일 로딩 (TAD §8, T6.3).
//!
//! `resources/training/profiles.toml`의 fast/standard/quality 3종을 파싱한다.
//! 사용자 오버라이드는 두지 않는다 — 프로파일 튜닝은 리소스 갱신으로.

use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const PROFILES_TOML: &str = include_str!("../../resources/training/profiles.toml");

/// TAD §5 `training_start.profile` 값과 1:1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileKind {
    Fast,
    Standard,
    Quality,
}

impl ProfileKind {
    pub fn key(self) -> &'static str {
        match self {
            ProfileKind::Fast => "fast",
            ProfileKind::Standard => "standard",
            ProfileKind::Quality => "quality",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub label_ko: String,
    pub estimate_ko: String,
    pub max_train_epochs: u32,
    pub network_dim: u32,
    pub network_alpha: u32,
    pub learning_rate: f64,
    /// 데이터셋 폴더 이름 {repeats}_{trigger}에 쓰임 (kohya 규약, D-009)
    pub repeats: u32,
    pub resolution: u32,
    pub train_batch_size: u32,
    pub sample_every_n_epochs: u32,
}

#[derive(Debug, Deserialize)]
struct ProfilesFile {
    fast: Profile,
    standard: Profile,
    quality: Profile,
}

/// 내장 profiles.toml에서 프로파일 로드.
pub fn load_profile(kind: ProfileKind) -> Result<Profile, AppError> {
    let file: ProfilesFile = toml::from_str(PROFILES_TOML).map_err(|e| {
        AppError::with_detail("E_PROFILE_PARSE", "학습 프로파일을 읽지 못했어요.", e)
    })?;
    Ok(match kind {
        ProfileKind::Fast => file.fast,
        ProfileKind::Standard => file.standard,
        ProfileKind::Quality => file.quality,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_profiles_parse_with_three_kinds() {
        for kind in [
            ProfileKind::Fast,
            ProfileKind::Standard,
            ProfileKind::Quality,
        ] {
            let p = load_profile(kind).unwrap();
            assert!(p.max_train_epochs > 0, "{kind:?}");
            assert!(p.repeats > 0, "{kind:?}");
            assert!(p.learning_rate > 0.0, "{kind:?}");
            assert!(!p.label_ko.is_empty(), "{kind:?}");
        }
    }

    #[test]
    fn quality_is_heavier_than_fast() {
        let fast = load_profile(ProfileKind::Fast).unwrap();
        let quality = load_profile(ProfileKind::Quality).unwrap();
        assert!(quality.max_train_epochs > fast.max_train_epochs);
        assert!(quality.network_dim > fast.network_dim);
    }

    #[test]
    fn profile_kind_deserializes_from_contract_strings() {
        // TAD §5: profile: fast|standard|quality
        let k: ProfileKind = serde_json::from_str(r#""fast""#).unwrap();
        assert_eq!(k, ProfileKind::Fast);
        assert!(serde_json::from_str::<ProfileKind>(r#""ultra""#).is_err());
    }
}
