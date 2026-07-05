//! 프로파일별 모델 카탈로그 (TAD §7).
//!
//! 다운로드 출처는 Hugging Face만 허용 (CLAUDE.md 절대 규칙 1).
//! 대상 경로는 앱 데이터 루트 기준 상대 경로 (TAD §3 폴더 구조).

use super::state::ModelProfile;

pub struct ModelSpec {
    /// Hugging Face resolve URL
    pub url: &'static str,
    /// 앱 데이터 루트 기준 저장 경로
    pub dest_rel: &'static str,
    /// 진행률 가중치용 대략 크기(bytes)
    pub approx_bytes: u64,
}

const GB: u64 = 1024 * 1024 * 1024;

/// standard: SDXL base + IP-Adapter(+이미지 인코더) (~10GB)
const STANDARD: &[ModelSpec] = &[
    ModelSpec {
        url: "https://huggingface.co/stabilityai/stable-diffusion-xl-base-1.0/resolve/main/sd_xl_base_1.0.safetensors",
        dest_rel: "models/checkpoints/sd_xl_base_1.0.safetensors",
        approx_bytes: 7 * GB,
    },
    ModelSpec {
        url: "https://huggingface.co/h94/IP-Adapter/resolve/main/sdxl_models/ip-adapter_sdxl.safetensors",
        dest_rel: "models/ipadapter/ip-adapter_sdxl.safetensors",
        approx_bytes: GB,
    },
    ModelSpec {
        url: "https://huggingface.co/h94/IP-Adapter/resolve/main/sdxl_models/image_encoder/model.safetensors",
        dest_rel: "models/clip_vision/clip_vision_bigG.safetensors",
        approx_bytes: 2 * GB,
    },
];

/// light: SD1.5 (8GB RAM 폴백, ~4GB)
const LIGHT: &[ModelSpec] = &[ModelSpec {
    url: "https://huggingface.co/Comfy-Org/stable-diffusion-v1-5-archive/resolve/main/v1-5-pruned-emaonly-fp16.safetensors",
    dest_rel: "models/checkpoints/v1-5-pruned-emaonly-fp16.safetensors",
    approx_bytes: 2 * GB,
}];

pub fn models_for(profile: ModelProfile) -> &'static [ModelSpec] {
    match profile {
        ModelProfile::Standard => STANDARD,
        ModelProfile::Light => LIGHT,
    }
}

/// RAM 크기로 기본 프로파일 제안 (TAD §7: RAM 감지로 기본값 제안).
/// 16GB 미만이면 light (02 PRD의 8GB 폴백 시나리오).
pub fn suggest_profile(ram_bytes: u64) -> ModelProfile {
    if ram_bytes >= 16 * GB {
        ModelProfile::Standard
    } else {
        ModelProfile::Light
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogs_are_nonempty_and_hf_only() {
        for profile in [ModelProfile::Standard, ModelProfile::Light] {
            let list = models_for(profile);
            assert!(!list.is_empty());
            for m in list {
                // 절대 규칙 1: 모델 다운로드는 Hugging Face만
                assert!(
                    m.url.starts_with("https://huggingface.co/"),
                    "허용되지 않은 출처: {}",
                    m.url
                );
                // 대상은 루트 기준 상대 경로 (TAD §3)
                assert!(
                    !m.dest_rel.starts_with('/'),
                    "절대 경로 금지: {}",
                    m.dest_rel
                );
                assert!(m.dest_rel.starts_with("models/"));
                assert!(m.approx_bytes > 0);
            }
        }
    }

    #[test]
    fn profile_suggestion_by_ram() {
        assert_eq!(suggest_profile(8 * GB), ModelProfile::Light);
        assert_eq!(suggest_profile(16 * GB), ModelProfile::Standard);
        assert_eq!(suggest_profile(32 * GB), ModelProfile::Standard);
    }
}
