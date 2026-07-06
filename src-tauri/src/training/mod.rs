//! LoRA 학습 데이터셋 준비 (TAD §2 `training/`, T6.2). 잡 러너(T6.3)도
//! 이 모듈에 추가된다.
//!
//! 데이터셋은 앱 데이터 루트의 `datasets/{id}/`에 이미지를 복사해 두고,
//! 캡션은 kohya sd-scripts 관례대로 같은 폴더에 `{basename}.txt`로 저장한다
//! (이미지 파일명과 1:1). styles.rs의 참조 이미지 복사 패턴과 동일.

use std::path::{Path, PathBuf};

use crate::error::AppError;

pub fn dataset_dir(data_root: &Path, id: &str) -> PathBuf {
    data_root.join("datasets").join(id)
}

/// 드롭된 절대 경로 이미지들을 datasets/{id}/로 복사하고 파일명 목록을 돌려준다.
pub fn copy_dataset_images(
    data_root: &Path,
    id: &str,
    sources: &[String],
) -> Result<Vec<String>, AppError> {
    let dir = dataset_dir(data_root, id);
    std::fs::create_dir_all(&dir)?;
    let mut files = Vec::new();
    for (i, src) in sources.iter().enumerate() {
        let src_path = Path::new(src);
        let ext = src_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let name = format!("img-{i:03}.{ext}");
        std::fs::copy(src_path, dir.join(&name)).map_err(|e| {
            AppError::with_detail(
                "E_DATASET_COPY",
                "이미지를 데이터셋에 복사하지 못했어요.",
                format!("{src}: {e}"),
            )
        })?;
        files.push(name);
    }
    Ok(files)
}

/// 캡션을 `{basename}.txt`로 저장(kohya sd-scripts 관례 — 이미지와 같은 폴더,
/// 같은 basename). file은 데이터셋 폴더 안의 파일명(경로 아님)이어야 한다.
pub fn save_captions(dir: &Path, items: &[(String, String)]) -> Result<(), AppError> {
    for (file, caption) in items {
        let stem = Path::new(file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file.as_str());
        std::fs::write(dir.join(format!("{stem}.txt")), caption)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_images_with_sequential_names_and_original_extensions() {
        let dir = tempfile::tempdir().unwrap();
        let src_png = dir.path().join("a.png");
        let src_jpg = dir.path().join("b.jpg");
        std::fs::write(&src_png, b"x").unwrap();
        std::fs::write(&src_jpg, b"y").unwrap();

        let files = copy_dataset_images(
            dir.path(),
            "ds1",
            &[
                src_png.to_string_lossy().into_owned(),
                src_jpg.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();

        assert_eq!(files, vec!["img-000.png", "img-001.jpg"]);
        let ds_dir = dataset_dir(dir.path(), "ds1");
        assert!(ds_dir.join("img-000.png").exists());
        assert!(ds_dir.join("img-001.jpg").exists());
    }

    #[test]
    fn copy_fails_with_detail_when_source_missing() {
        let dir = tempfile::tempdir().unwrap();
        let err =
            copy_dataset_images(dir.path(), "ds1", &["/no/such/file.png".to_string()]).unwrap_err();
        assert_eq!(err.code, "E_DATASET_COPY");
    }

    #[test]
    fn save_captions_writes_txt_sidecar_matching_basename() {
        let dir = tempfile::tempdir().unwrap();
        let ds_dir = dataset_dir(dir.path(), "ds1");
        std::fs::create_dir_all(&ds_dir).unwrap();

        save_captions(
            &ds_dir,
            &[
                ("img-000.png".to_string(), "cat, flat color".to_string()),
                ("img-001.jpg".to_string(), "dog".to_string()),
            ],
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(ds_dir.join("img-000.txt")).unwrap(),
            "cat, flat color"
        );
        assert_eq!(
            std::fs::read_to_string(ds_dir.join("img-001.txt")).unwrap(),
            "dog"
        );
    }
}
