//! export_image (TAD §5, F-4.2).
//!
//! T2.4 범위: PNG 원본 복사 다운로드. jpg/webp 변환은 갤러리 상세(T3.2),
//! 투명 배경(배경 제거)은 T2.4b에서 추가한다.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::{AppHandle, Manager, State};

use crate::db::Db;
use crate::error::AppError;
use crate::paths;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArgs {
    pub id: String,
    /// png | jpg | webp (T2.4는 png만)
    pub format: String,
    pub transparent: Option<bool>,
    pub dest_dir: String,
}

/// 파일명에 쓸 수 있게 정리: 한글/영숫자/-/_만 유지, 공백→-, 최대 40자.
pub fn sanitize_filename_part(text: &str) -> String {
    let cleaned: String = text
        .trim()
        .chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '가'..='힣'))
        .collect();
    let truncated: String = cleaned.chars().take(40).collect();
    if truncated.is_empty() {
        "image".to_string()
    } else {
        truncated
    }
}

/// dest_dir 안에서 겹치지 않는 경로: `{base}.{ext}`, 겹치면 `{base}-2.{ext}`…
pub fn unique_path(dest_dir: &Path, base: &str, ext: &str) -> PathBuf {
    let first = dest_dir.join(format!("{base}.{ext}"));
    if !first.exists() {
        return first;
    }
    for n in 2..1000 {
        let candidate = dest_dir.join(format!("{base}-{n}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    // 사실상 도달 불가 — 마지막 후보 반환
    dest_dir.join(format!("{base}-999.{ext}"))
}

#[tauri::command]
pub async fn export_image(
    app: AppHandle,
    db: State<'_, Db>,
    args: ExportArgs,
) -> Result<String, AppError> {
    if args.format != "png" {
        return Err(AppError::new(
            "E_FORMAT_UNSUPPORTED",
            "지금은 PNG로만 저장할 수 있어요. JPG·WebP는 갤러리에서 곧 지원돼요.",
        ));
    }
    if args.transparent == Some(true) {
        return Err(AppError::new(
            "E_TRANSPARENT_UNSUPPORTED",
            "투명 배경 저장은 아직 준비 중이에요.",
        ));
    }

    let gen = db
        .get_generation(&args.id)
        .await
        .map_err(|e| AppError::with_detail("E_DB", "히스토리를 읽지 못했어요.", e))?
        .ok_or_else(|| AppError::new("E_GEN_NOT_FOUND", "이미지를 찾을 수 없어요."))?;

    let data_root = paths::app_data_root(
        &app.path()
            .data_dir()
            .map_err(|e| AppError::with_detail("E_PATH", "앱 데이터 폴더를 찾지 못했어요.", e))?,
    );
    let src = data_root.join(&gen.image_path);
    if !src.exists() {
        return Err(AppError::with_detail(
            "E_IMAGE_MISSING",
            "원본 이미지 파일이 없어요. 저장 위치를 옮겼다면 설정에서 확인해 주세요.",
            gen.image_path.clone(),
        ));
    }

    let dest_dir = PathBuf::from(&args.dest_dir);
    std::fs::create_dir_all(&dest_dir)?;
    // 파일명 규칙(기본값): {키워드}_{시드}.png — 규칙 설정 UI는 T7.1
    let base = format!(
        "{}_{}",
        sanitize_filename_part(gen.keyword_ko.as_deref().unwrap_or("image")),
        gen.seed
    );
    let dest = unique_path(&dest_dir, &base, "png");
    std::fs::copy(&src, &dest).map_err(|e| {
        AppError::with_detail(
            "E_EXPORT_COPY",
            "이미지를 저장하지 못했어요.",
            format!("{} -> {}: {e}", src.display(), dest.display()),
        )
    })?;
    Ok(dest.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_keeps_korean_and_alnum_and_replaces_spaces() {
        assert_eq!(sanitize_filename_part("통나무집"), "통나무집");
        assert_eq!(sanitize_filename_part("log cabin 2"), "log-cabin-2");
        assert_eq!(sanitize_filename_part("  a/b\\c:d*e  "), "abcde");
        assert_eq!(sanitize_filename_part("!!!"), "image");
        assert_eq!(sanitize_filename_part(""), "image");
    }

    #[test]
    fn sanitize_truncates_to_40_chars() {
        let long = "가".repeat(80);
        assert_eq!(sanitize_filename_part(&long).chars().count(), 40);
    }

    #[test]
    fn unique_path_appends_counter_on_collision() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            unique_path(dir.path(), "통나무집_42", "png"),
            dir.path().join("통나무집_42.png")
        );
        std::fs::write(dir.path().join("통나무집_42.png"), b"x").unwrap();
        assert_eq!(
            unique_path(dir.path(), "통나무집_42", "png"),
            dir.path().join("통나무집_42-2.png")
        );
        std::fs::write(dir.path().join("통나무집_42-2.png"), b"x").unwrap();
        assert_eq!(
            unique_path(dir.path(), "통나무집_42", "png"),
            dir.path().join("통나무집_42-3.png")
        );
    }
}
