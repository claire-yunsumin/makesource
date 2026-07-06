//! presets_get / presets_save (TAD §5).
//!
//! 앱 데이터 루트의 `presets.json`(사용자)이 있으면 그것을, 없으면 내장 기본값을
//! 로드해 프리셋 목록을 반환한다. `presets_save`는 편집 전 상태를 history에
//! 스냅샷으로 남기고 version을 올린다(T5.1) — 복원도 이 커맨드를 재사용한다
//! (프론트에서 히스토리 스냅샷 값을 다시 저장).

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::paths;
use crate::prompt::presets::{load_presets, upsert_preset, Preset};

fn data_root(app: &AppHandle) -> Result<std::path::PathBuf, AppError> {
    Ok(paths::app_data_root(&app.path().data_dir().map_err(
        |e| AppError::with_detail("E_PATH", "앱 데이터 폴더를 찾지 못했어요.", e),
    )?))
}

#[tauri::command]
pub async fn presets_get(app: AppHandle) -> Result<Vec<Preset>, AppError> {
    Ok(load_presets(&data_root(&app)?)?.presets)
}

#[tauri::command]
pub async fn presets_save(app: AppHandle, preset: Preset) -> Result<(), AppError> {
    let saved_at = chrono::Utc::now().timestamp_millis();
    upsert_preset(&data_root(&app)?, preset, saved_at)?;
    Ok(())
}
