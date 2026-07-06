//! presets_get (TAD §5).
//!
//! 앱 데이터 루트의 `presets.json`(사용자)이 있으면 그것을, 없으면 내장 기본값을
//! 로드해 프리셋 목록을 반환한다. 편집·저장(presets_save)과 버전 관리는
//! 프리셋 편집기(T5.1)에서 추가한다.

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::paths;
use crate::prompt::presets::{load_presets, Preset};

#[tauri::command]
pub async fn presets_get(app: AppHandle) -> Result<Vec<Preset>, AppError> {
    let data_root = paths::app_data_root(
        &app.path()
            .data_dir()
            .map_err(|e| AppError::with_detail("E_PATH", "앱 데이터 폴더를 찾지 못했어요.", e))?,
    );
    Ok(load_presets(&data_root)?.presets)
}
