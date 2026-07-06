//! presets_get / presets_save / presets_export / presets_import (TAD §5).
//!
//! 앱 데이터 루트의 `presets.json`(사용자)이 있으면 그것을, 없으면 내장 기본값을
//! 로드해 프리셋 목록을 반환한다. `presets_save`는 편집 전 상태를 history에
//! 스냅샷으로 남기고 version을 올린다(T5.1) — 복원도 이 커맨드를 재사용한다
//! (프론트에서 히스토리 스냅샷 값을 다시 저장). `presets_export`/`presets_import`는
//! destPath/srcPath를 프론트(파일 다이얼로그)에서 받아 파일 IO는 여기서만
//! 수행한다(T5.3). 가져오기도 presets_save와 같은 upsert 경로를 타 버전이
//! 올라가고 기존 상태가 history에 남는다.

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::paths;
use crate::prompt::presets::{export_presets, import_presets, load_presets, upsert_preset, Preset};

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

/// dest_path는 프론트에서 저장 다이얼로그로 고른 절대 경로.
#[tauri::command]
pub async fn presets_export(app: AppHandle, dest_path: String) -> Result<(), AppError> {
    export_presets(&data_root(&app)?, std::path::Path::new(&dest_path))
}

/// src_path는 프론트에서 열기 다이얼로그로 고른 절대 경로.
#[tauri::command]
pub async fn presets_import(app: AppHandle, src_path: String) -> Result<Vec<Preset>, AppError> {
    let saved_at = chrono::Utc::now().timestamp_millis();
    import_presets(&data_root(&app)?, std::path::Path::new(&src_path), saved_at)
}
