//! 설정 화면 커맨드 (TAD §5, T7.1) — settings.json, 모델 목록·캐시, 라이선스 BOM.
//! 얇게 유지 — 로직은 settings/storage 모듈에.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::paths;
use crate::settings::AppSettings;
use crate::storage::{self, ModelEntry};

/// 오픈소스 라이선스 BOM (resources/licenses.json — 빌드에 포함).
const LICENSES_JSON: &str = include_str!("../../resources/licenses.json");

fn data_root(app: &AppHandle) -> Result<std::path::PathBuf, AppError> {
    let base = app
        .path()
        .data_dir()
        .map_err(|e| AppError::with_detail("E_PATH", "앱 데이터 폴더를 찾지 못했어요.", e))?;
    Ok(paths::app_data_root(&base))
}

#[tauri::command]
pub async fn settings_get(app: AppHandle) -> Result<AppSettings, AppError> {
    Ok(AppSettings::load(&data_root(&app)?))
}

#[tauri::command]
pub async fn settings_save(app: AppHandle, settings: AppSettings) -> Result<(), AppError> {
    settings
        .save(&data_root(&app)?)
        .map_err(|e| AppError::with_detail("E_SETTINGS_SAVE", "설정을 저장하지 못했어요.", e))
}

#[tauri::command]
pub async fn models_list(app: AppHandle) -> Result<Vec<ModelEntry>, AppError> {
    Ok(storage::scan_models(&data_root(&app)?))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    pub size_bytes: u64,
}

#[tauri::command]
pub async fn cache_stats(app: AppHandle) -> Result<CacheStats, AppError> {
    Ok(CacheStats {
        size_bytes: storage::cache_size(&data_root(&app)?),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheClearResult {
    pub freed_bytes: u64,
}

#[tauri::command]
pub async fn cache_clear(app: AppHandle) -> Result<CacheClearResult, AppError> {
    let freed = storage::clear_cache(&data_root(&app)?)
        .map_err(|e| AppError::with_detail("E_CACHE_CLEAR", "캐시를 정리하지 못했어요.", e))?;
    Ok(CacheClearResult { freed_bytes: freed })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseEntry {
    pub name: String,
    pub license: String,
    pub url: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
struct LicensesFile {
    entries: Vec<LicenseEntry>,
}

#[tauri::command]
pub async fn licenses_get() -> Result<Vec<LicenseEntry>, AppError> {
    let file: LicensesFile = serde_json::from_str(LICENSES_JSON)
        .map_err(|e| AppError::with_detail("E_LICENSES", "라이선스 목록을 읽지 못했어요.", e))?;
    Ok(file.entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_licenses_json_parses_and_is_not_empty() {
        let file: LicensesFile = serde_json::from_str(LICENSES_JSON).unwrap();
        assert!(!file.entries.is_empty());
        // GPL은 별도 프로세스(ComfyUI)만 허용 (CLAUDE.md 절대 규칙 2) — 표기 확인
        let gpl: Vec<_> = file
            .entries
            .iter()
            .filter(|e| e.license.contains("GPL"))
            .collect();
        assert!(gpl.iter().all(|e| e.license.contains("별도 프로세스")));
    }
}
