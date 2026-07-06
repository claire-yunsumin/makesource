//! 스타일 저장소 — 데이터 루트 `styles.json` (TAD §3.3).
//!
//! kind=essence(에센스 프롬프트 + 참조 이미지 + IP-Adapter 강도) 또는
//! kind=lora(LoRA 경로 + weight + 트리거워드). 참조 이미지는 style_save가
//! `styles/{id}/`로 복사하고 루트 기준 상대 경로로 저장한다 (CLAUDE.md 주의사항).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    pub id: String,
    pub name: String,
    /// "essence" | "lora"
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub essence_prompt: Option<String>,
    /// 데이터 루트 기준 상대 경로 (IP-Adapter 입력)
    #[serde(default)]
    pub reference_images: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_adapter_weight: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lora_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lora_weight: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_word: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb: Option<String>,
    /// unix ms
    pub created_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StylesFile {
    pub styles: Vec<Style>,
}

fn styles_path(data_root: &Path) -> std::path::PathBuf {
    data_root.join("styles.json")
}

/// styles.json 로드 (없으면 빈 목록).
pub fn load_styles(data_root: &Path) -> Result<StylesFile, AppError> {
    let path = styles_path(data_root);
    if !path.exists() {
        return Ok(StylesFile::default());
    }
    let text = std::fs::read_to_string(&path)?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::with_detail("E_STYLES_PARSE", "스타일 파일을 읽지 못했어요.", e))
}

/// 원자적 저장 (tmp 기록 후 rename — 쓰다 죽어도 원본 보존).
fn save_file(data_root: &Path, file: &StylesFile) -> Result<(), AppError> {
    let path = styles_path(data_root);
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(file)
        .map_err(|e| AppError::with_detail("E_STYLES_WRITE", "스타일을 저장하지 못했어요.", e))?;
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// upsert: 같은 id가 있으면 교체, 없으면 추가.
pub fn upsert_style(data_root: &Path, style: Style) -> Result<(), AppError> {
    let mut file = load_styles(data_root)?;
    match file.styles.iter_mut().find(|s| s.id == style.id) {
        Some(slot) => *slot = style,
        None => file.styles.push(style),
    }
    save_file(data_root, &file)
}

/// 삭제: styles.json에서 제거 + 참조 이미지 폴더(styles/{id}) 정리.
pub fn delete_style(data_root: &Path, id: &str) -> Result<(), AppError> {
    let mut file = load_styles(data_root)?;
    let before = file.styles.len();
    file.styles.retain(|s| s.id != id);
    if file.styles.len() == before {
        return Err(AppError::new(
            "E_STYLE_NOT_FOUND",
            "스타일을 찾을 수 없어요.",
        ));
    }
    save_file(data_root, &file)?;
    let dir = data_root.join("styles").join(id);
    if dir.exists() {
        let _ = std::fs::remove_dir_all(&dir); // 정리 실패는 치명적이지 않음
    }
    Ok(())
}

/// 참조 이미지들을 `styles/{id}/ref-N.{ext}`로 복사하고 상대 경로 목록을 돌려준다.
pub fn copy_reference_images(
    data_root: &Path,
    id: &str,
    sources: &[String],
) -> Result<Vec<String>, AppError> {
    let dir_rel = format!("styles/{id}");
    let dir = data_root.join(&dir_rel);
    std::fs::create_dir_all(&dir)?;
    let mut rels = Vec::new();
    for (i, src) in sources.iter().enumerate() {
        let src_path = Path::new(src);
        let ext = src_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let rel = format!("{dir_rel}/ref-{i}.{ext}");
        std::fs::copy(src_path, data_root.join(&rel)).map_err(|e| {
            AppError::with_detail(
                "E_REF_COPY",
                "참조 이미지를 복사하지 못했어요.",
                format!("{src}: {e}"),
            )
        })?;
        rels.push(rel);
    }
    Ok(rels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn essence_style(id: &str) -> Style {
        Style {
            id: id.to_string(),
            name: "우리 브랜드".to_string(),
            kind: "essence".to_string(),
            essence_prompt: Some("flat color, simple background".to_string()),
            reference_images: vec![],
            ip_adapter_weight: Some(0.6),
            lora_path: None,
            lora_weight: None,
            trigger_word: None,
            thumb: None,
            created_at: 1_700_000_000_000,
        }
    }

    #[test]
    fn missing_file_is_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_styles(dir.path()).unwrap().styles.is_empty());
    }

    #[test]
    fn upsert_roundtrip_and_replace() {
        let dir = tempfile::tempdir().unwrap();
        upsert_style(dir.path(), essence_style("s1")).unwrap();
        upsert_style(dir.path(), essence_style("s2")).unwrap();
        assert_eq!(load_styles(dir.path()).unwrap().styles.len(), 2);

        // 같은 id는 교체
        let mut updated = essence_style("s1");
        updated.name = "바뀐 이름".to_string();
        upsert_style(dir.path(), updated).unwrap();
        let file = load_styles(dir.path()).unwrap();
        assert_eq!(file.styles.len(), 2);
        assert_eq!(
            file.styles.iter().find(|s| s.id == "s1").unwrap().name,
            "바뀐 이름"
        );
        // TAD §3.3 camelCase 직렬화 확인
        let text = std::fs::read_to_string(dir.path().join("styles.json")).unwrap();
        assert!(text.contains("essencePrompt"));
        assert!(text.contains("ipAdapterWeight"));
    }

    #[test]
    fn delete_removes_entry_and_ref_dir() {
        let dir = tempfile::tempdir().unwrap();
        upsert_style(dir.path(), essence_style("s1")).unwrap();
        std::fs::create_dir_all(dir.path().join("styles/s1")).unwrap();
        delete_style(dir.path(), "s1").unwrap();
        assert!(load_styles(dir.path()).unwrap().styles.is_empty());
        assert!(!dir.path().join("styles/s1").exists());
        assert_eq!(
            delete_style(dir.path(), "s1").unwrap_err().code,
            "E_STYLE_NOT_FOUND"
        );
    }

    #[test]
    fn copies_reference_images_to_style_dir() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.png");
        std::fs::write(&src, b"img").unwrap();
        let rels =
            copy_reference_images(dir.path(), "s1", &[src.to_string_lossy().into_owned()]).unwrap();
        assert_eq!(rels, vec!["styles/s1/ref-0.png".to_string()]);
        assert!(dir.path().join("styles/s1/ref-0.png").exists());
    }
}
