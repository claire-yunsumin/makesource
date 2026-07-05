//! 프리셋 로딩 (TAD §3.2).
//!
//! 앱 데이터 루트의 `presets.json`이 있으면 사용, 없으면 내장 기본값
//! (`resources/presets.default.json`)을 반환한다. 편집·버전 관리는 T5.1.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const DEFAULT_PRESETS: &str = include_str!("../../resources/presets.default.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetParams {
    pub steps: u32,
    pub cfg: f64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub label: serde_json::Value,
    pub version: u32,
    #[serde(default)]
    pub history: Vec<serde_json::Value>,
    #[serde(default)]
    pub success_criteria: String,
    pub prefix: String,
    pub suffix: String,
    pub negative: String,
    pub params: PresetParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetFile {
    pub schema_version: u32,
    pub presets: Vec<Preset>,
}

/// presets.json 로드 (없으면 내장 기본값).
pub fn load_presets(data_root: &Path) -> Result<PresetFile, AppError> {
    let user_path = data_root.join("presets.json");
    let text = if user_path.exists() {
        std::fs::read_to_string(&user_path)?
    } else {
        DEFAULT_PRESETS.to_string()
    };
    serde_json::from_str(&text)
        .map_err(|e| AppError::with_detail("E_PRESET_PARSE", "프리셋 파일을 읽지 못했어요.", e))
}

/// id로 프리셋 찾기.
pub fn find_preset(file: &PresetFile, id: &str) -> Result<Preset, AppError> {
    file.presets
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| AppError::with_detail("E_PRESET_NOT_FOUND", "프리셋을 찾을 수 없어요.", id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_presets_parse_with_six_entries() {
        let file: PresetFile = serde_json::from_str(DEFAULT_PRESETS).unwrap();
        assert_eq!(file.schema_version, 1);
        assert_eq!(file.presets.len(), 6, "T2.2: 기본 프리셋 6종");
        // TAD §3.2 예시 프리셋 존재
        let storybook = file.presets.iter().find(|p| p.id == "storybook").unwrap();
        assert_eq!(storybook.params.width, 1024);
        assert!(!storybook.prefix.is_empty());
        assert!(!storybook.negative.is_empty());
    }

    #[test]
    fn load_falls_back_to_defaults_and_finds_by_id() {
        let dir = tempfile::tempdir().unwrap();
        let file = load_presets(dir.path()).unwrap();
        assert!(find_preset(&file, "storybook").is_ok());
        let err = find_preset(&file, "no-such").unwrap_err();
        assert_eq!(err.code, "E_PRESET_NOT_FOUND");
    }

    #[test]
    fn user_presets_override_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("presets.json"),
            r#"{"schemaVersion":1,"presets":[{"id":"mine","label":{},"version":1,"prefix":"p","suffix":"s","negative":"n","params":{"steps":20,"cfg":6.0,"width":512,"height":512}}]}"#,
        )
        .unwrap();
        let file = load_presets(dir.path()).unwrap();
        assert_eq!(file.presets.len(), 1);
        assert!(find_preset(&file, "mine").is_ok());
    }
}
