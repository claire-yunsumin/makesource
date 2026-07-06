//! history_toggle_favorite (TAD §5). 목록·검색(history_list)은 갤러리(M3)에서.

use tauri::State;

use crate::db::Db;
use crate::error::AppError;

#[tauri::command]
pub async fn history_toggle_favorite(db: State<'_, Db>, id: String) -> Result<(), AppError> {
    let gen = db
        .get_generation(&id)
        .await
        .map_err(|e| AppError::with_detail("E_DB", "히스토리를 읽지 못했어요.", e))?
        .ok_or_else(|| AppError::new("E_GEN_NOT_FOUND", "이미지를 찾을 수 없어요."))?;
    db.set_favorite(&id, !gen.favorite)
        .await
        .map_err(|e| AppError::with_detail("E_DB", "즐겨찾기를 저장하지 못했어요.", e))?;
    Ok(())
}
