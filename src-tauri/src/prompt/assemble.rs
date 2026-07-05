//! 프롬프트 조립 규칙 (TAD §4).
//!
//! 최종 = `[preset.prefix] + [스타일 트리거워드?] + [키워드(영문)] + [style.essencePrompt?] + [preset.suffix]`
//! negative = preset.negative + 전역 안전 네거티브.
//! 한→영 변환(§4 ①②③)은 T2.3에서 — 여기서는 입력 키워드를 그대로 사용한다.

/// 스타일이 프롬프트에 기여하는 조각 (styles.json — TAD §3.3).
#[derive(Debug, Clone, Default)]
pub struct StyleFragments {
    /// LoRA 트리거워드 (kind=lora)
    pub trigger_word: Option<String>,
    /// 에센스 프롬프트 (kind=essence)
    pub essence_prompt: Option<String>,
}

/// TAD §4 순서로 최종 프롬프트를 조립한다. 빈 조각은 건너뛴다.
pub fn assemble_prompt(
    prefix: &str,
    keyword_en: &str,
    style: &StyleFragments,
    suffix: &str,
) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in [
        prefix,
        style.trigger_word.as_deref().unwrap_or(""),
        keyword_en,
        style.essence_prompt.as_deref().unwrap_or(""),
        suffix,
    ] {
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed);
        }
    }
    parts.join(", ")
}

/// negative = preset.negative + 전역 안전 네거티브 (중복 제거는 하지 않음 — 단순 연결).
pub fn assemble_negative(preset_negative: &str, global_safety: &str) -> String {
    match (preset_negative.trim(), global_safety.trim()) {
        ("", g) => g.to_string(),
        (p, "") => p.to_string(),
        (p, g) => format!("{p}, {g}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_in_tad_order() {
        let style = StyleFragments {
            trigger_word: Some("brandstyle".into()),
            essence_prompt: Some("flat vector illustration".into()),
        };
        let out = assemble_prompt(
            "cinematic illustration of",
            "log cabin",
            &style,
            "soft pastel colors",
        );
        assert_eq!(
            out,
            "cinematic illustration of, brandstyle, log cabin, flat vector illustration, soft pastel colors"
        );
    }

    #[test]
    fn skips_empty_fragments() {
        let out = assemble_prompt("prefix", "cabin", &StyleFragments::default(), "");
        assert_eq!(out, "prefix, cabin");
    }

    #[test]
    fn negative_concatenates_with_global_safety() {
        assert_eq!(
            assemble_negative("text, watermark", "nsfw, violence"),
            "text, watermark, nsfw, violence"
        );
        assert_eq!(assemble_negative("", "nsfw"), "nsfw");
        assert_eq!(assemble_negative("text", ""), "text");
    }
}
