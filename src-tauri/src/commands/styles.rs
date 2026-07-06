//! styles_list / style_save / style_delete (TAD §5, §3.3).

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::paths;
use crate::styles::{copy_reference_images, delete_style, load_styles, upsert_style, Style};

fn data_root(app: &AppHandle) -> Result<std::path::PathBuf, AppError> {
    Ok(paths::app_data_root(&app.path().data_dir().map_err(
        |e| AppError::with_detail("E_PATH", "앱 데이터 폴더를 찾지 못했어요.", e),
    )?))
}

#[tauri::command]
pub async fn styles_list(app: AppHandle) -> Result<Vec<Style>, AppError> {
    Ok(load_styles(&data_root(&app)?)?.styles)
}

/// 저장. referenceImages에 절대 경로(마법사가 고른 원본)가 있으면
/// `styles/{id}/`로 복사하고 루트 기준 상대 경로로 바꿔 저장한다.
#[tauri::command]
pub async fn style_save(app: AppHandle, mut style: Style) -> Result<(), AppError> {
    let root = data_root(&app)?;
    let absolute: Vec<String> = style
        .reference_images
        .iter()
        .filter(|p| std::path::Path::new(p).is_absolute())
        .cloned()
        .collect();
    if !absolute.is_empty() {
        let mut rels = copy_reference_images(&root, &style.id, &absolute)?;
        // 이미 상대인 항목(기존 저장분)은 유지
        let mut keep: Vec<String> = style
            .reference_images
            .iter()
            .filter(|p| !std::path::Path::new(p).is_absolute())
            .cloned()
            .collect();
        keep.append(&mut rels);
        style.reference_images = keep;
        // 썸네일 미지정이면 첫 참조 이미지로
        if style.thumb.is_none() {
            style.thumb = style.reference_images.first().cloned();
        }
    }
    upsert_style(&root, style)
}

#[tauri::command]
pub async fn style_delete(app: AppHandle, id: String) -> Result<(), AppError> {
    delete_style(&data_root(&app)?, &id)
}
