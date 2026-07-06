//! translate_keyword (TAD §5) — 고급 패널의 변환 결과 미리보기용.
//!
//! 생성 파이프라인은 이 command를 거치지 않고 run_generation 안에서
//! 같은 로직(prompt::translate)을 직접 호출한다.

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::paths;
use crate::prompt::translate::{translate_keyword as translate_impl, Translation};

#[tauri::command]
pub async fn translate_keyword(app: AppHandle, keyword: String) -> Result<Translation, AppError> {
    let data_root = paths::app_data_root(
        &app.path()
            .data_dir()
            .map_err(|e| AppError::with_detail("E_PATH", "앱 데이터 폴더를 찾지 못했어요.", e))?,
    );
    Ok(translate_impl(&data_root, &keyword).await)
}
