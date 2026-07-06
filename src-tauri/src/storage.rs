//! 저장 공간 조회 (04 §4.5 모델 관리, F-5.3) — T7.1.
//!
//! `models/` 아래 설치된 모델의 목록·용량과 캐시(models/hf) 크기를 계산한다.
//! 사용자가 고른 모델(체크포인트·LoRA 등)은 파일 단위로, 보조 모델은 폴더
//! 단위로 묶어서 보여준다.

use std::path::Path;

use serde::Serialize;

/// 파일 단위로 나열하는 카테고리 (사용자가 관리하는 모델).
const FILE_CATEGORIES: [&str; 4] = ["checkpoints", "loras", "ipadapter", "clip_vision"];
/// 폴더 단위로 합산하는 카테고리 (앱이 관리하는 보조 모델·캐시).
const DIR_CATEGORIES: [&str; 3] = ["argos", "rembg", "hf"];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    /// 파일명(파일 단위) 또는 폴더명(폴더 단위)
    pub name: String,
    /// models/ 아래 폴더명 (checkpoints, loras, …)
    pub category: String,
    pub size_bytes: u64,
}

/// 폴더 전체 크기 (재귀). 읽지 못하는 항목은 0으로 친다.
pub fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            match e.metadata() {
                Ok(m) if m.is_dir() => dir_size(&p),
                Ok(m) => m.len(),
                Err(_) => 0,
            }
        })
        .sum()
}

/// `models/` 아래 설치된 모델 목록 (카테고리 순서 고정, 이름순 정렬).
pub fn scan_models(data_root: &Path) -> Vec<ModelEntry> {
    let models = data_root.join("models");
    let mut out = Vec::new();

    for category in FILE_CATEGORIES {
        let dir = models.join(category);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut files: Vec<ModelEntry> = entries
            .flatten()
            .filter_map(|e| {
                let meta = e.metadata().ok()?;
                if !meta.is_file() {
                    return None;
                }
                let name = e.file_name().to_string_lossy().to_string();
                // .DS_Store 등 숨김 파일 제외
                if name.starts_with('.') {
                    return None;
                }
                Some(ModelEntry {
                    name,
                    category: category.to_string(),
                    size_bytes: meta.len(),
                })
            })
            .collect();
        files.sort_by(|a, b| a.name.cmp(&b.name));
        out.extend(files);
    }

    for category in DIR_CATEGORIES {
        let dir = models.join(category);
        if !dir.is_dir() {
            continue;
        }
        out.push(ModelEntry {
            name: category.to_string(),
            category: category.to_string(),
            size_bytes: dir_size(&dir),
        });
    }

    out
}

/// 캐시(models/hf — 에센스 분석용 HF 캐시, 재다운로드 가능) 크기.
pub fn cache_size(data_root: &Path) -> u64 {
    dir_size(&data_root.join("models").join("hf"))
}

/// 캐시 비우기. 비운 바이트 수를 돌려준다. 폴더가 없으면 0.
pub fn clear_cache(data_root: &Path) -> std::io::Result<u64> {
    let dir = data_root.join("models").join("hf");
    if !dir.is_dir() {
        return Ok(0);
    }
    let freed = dir_size(&dir);
    std::fs::remove_dir_all(&dir)?;
    // 다음 다운로드가 바로 쓸 수 있게 빈 폴더는 남겨둔다
    std::fs::create_dir_all(&dir)?;
    Ok(freed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, bytes: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![0u8; bytes]).unwrap();
    }

    #[test]
    fn scan_lists_files_per_category_and_dirs_aggregated() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(&root.join("models/checkpoints/sdxl.safetensors"), 10);
        write(&root.join("models/checkpoints/.DS_Store"), 1);
        write(&root.join("models/loras/mystyle.safetensors"), 5);
        write(&root.join("models/hf/blobs/a"), 3);
        write(&root.join("models/hf/blobs/b"), 4);

        let entries = scan_models(root);
        assert_eq!(
            entries,
            vec![
                ModelEntry {
                    name: "sdxl.safetensors".into(),
                    category: "checkpoints".into(),
                    size_bytes: 10,
                },
                ModelEntry {
                    name: "mystyle.safetensors".into(),
                    category: "loras".into(),
                    size_bytes: 5,
                },
                ModelEntry {
                    name: "hf".into(),
                    category: "hf".into(),
                    size_bytes: 7,
                },
            ],
        );
    }

    #[test]
    fn scan_of_empty_root_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(scan_models(dir.path()).is_empty());
    }

    #[test]
    fn clear_cache_frees_and_keeps_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(&root.join("models/hf/blobs/a"), 8);

        assert_eq!(cache_size(root), 8);
        assert_eq!(clear_cache(root).unwrap(), 8);
        assert_eq!(cache_size(root), 0);
        assert!(root.join("models/hf").is_dir());

        // 캐시 폴더가 아예 없어도 에러 없이 0
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(clear_cache(empty.path()).unwrap(), 0);
    }
}
