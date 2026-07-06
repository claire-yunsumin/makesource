//! 앱 설정 (settings.json, TAD §3) — T7.1.
//!
//! 데이터 루트의 `settings.json`에 저장한다. 파일이 없거나 손상되면 기본값.
//! 현재 항목은 전역 안전 네거티브뿐 — 생성 시 preset.negative 뒤에 붙는다
//! (prompt::assemble::assemble_negative).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 기본 안전 네거티브 (설정에서 편집 가능).
pub const DEFAULT_SAFE_NEGATIVE: &str = "nsfw, nudity, gore, violence";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub safe_negative: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            safe_negative: DEFAULT_SAFE_NEGATIVE.to_string(),
        }
    }
}

pub fn settings_path(data_root: &Path) -> PathBuf {
    data_root.join("settings.json")
}

impl AppSettings {
    /// 로드. 없거나 손상 시 기본값 (부트스트랩 상태 파일과 같은 정책).
    pub fn load(data_root: &Path) -> Self {
        std::fs::read_to_string(settings_path(data_root))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, data_root: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(data_root)?;
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(settings_path(data_root), json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_corrupt_file_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(AppSettings::load(dir.path()), AppSettings::default());

        std::fs::write(settings_path(dir.path()), "{잘못된 json").unwrap();
        assert_eq!(AppSettings::load(dir.path()), AppSettings::default());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let s = AppSettings {
            safe_negative: "watermark, text".to_string(),
        };
        s.save(dir.path()).unwrap();
        assert_eq!(AppSettings::load(dir.path()), s);
    }

    #[test]
    fn unknown_fields_are_ignored_and_missing_fields_defaulted() {
        let dir = tempfile::tempdir().unwrap();
        // 미래 버전이 쓴 파일(모르는 필드)도 읽을 수 있어야 한다
        std::fs::write(settings_path(dir.path()), r#"{"futureField": 1}"#).unwrap();
        assert_eq!(
            AppSettings::load(dir.path()).safe_negative,
            DEFAULT_SAFE_NEGATIVE
        );
    }
}
