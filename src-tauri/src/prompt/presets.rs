//! 프리셋 로딩·저장·버전 관리 (TAD §3.2).
//!
//! 앱 데이터 루트의 `presets.json`이 있으면 사용, 없으면 내장 기본값
//! (`resources/presets.default.json`)을 반환한다. 저장 시 편집 전 상태를
//! `history`에 스냅샷으로 남기고 `version`을 올린다(T5.1). 복원은 히스토리
//! 스냅샷 값으로 다시 저장하는 것과 동일하다 — 별도 커맨드 없이 `presets_save`
//! 재사용(현재 버전도 스냅샷으로 보존됨).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const DEFAULT_PRESETS: &str = include_str!("../../resources/presets.default.json");

/// 가져오기에서 검증하는 presets.json `schemaVersion` (TAD §3.2). 스키마가
/// 바뀌면 마이그레이션을 추가하고 이 값을 올릴 것.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetParams {
    pub steps: u32,
    pub cfg: f64,
    pub width: u32,
    pub height: u32,
}

/// 저장 시점의 편집 가능 필드 스냅샷 (TAD §3.2 `history[]`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetSnapshot {
    pub version: u32,
    pub label: serde_json::Value,
    #[serde(default)]
    pub success_criteria: String,
    pub prefix: String,
    pub suffix: String,
    pub negative: String,
    pub params: PresetParams,
    /// unix ms
    pub saved_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub label: serde_json::Value,
    pub version: u32,
    #[serde(default)]
    pub history: Vec<PresetSnapshot>,
    #[serde(default)]
    pub success_criteria: String,
    pub prefix: String,
    pub suffix: String,
    pub negative: String,
    pub params: PresetParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetFile {
    pub schema_version: u32,
    pub presets: Vec<Preset>,
}

fn presets_path(data_root: &Path) -> std::path::PathBuf {
    data_root.join("presets.json")
}

/// presets.json 로드 (없으면 내장 기본값).
pub fn load_presets(data_root: &Path) -> Result<PresetFile, AppError> {
    let user_path = presets_path(data_root);
    let text = if user_path.exists() {
        std::fs::read_to_string(&user_path)?
    } else {
        DEFAULT_PRESETS.to_string()
    };
    serde_json::from_str(&text)
        .map_err(|e| AppError::with_detail("E_PRESET_PARSE", "프리셋 파일을 읽지 못했어요.", e))
}

/// 원자적 저장 (tmp 기록 후 rename — styles.rs와 동일 패턴).
fn save_file(data_root: &Path, file: &PresetFile) -> Result<(), AppError> {
    let path = presets_path(data_root);
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(file)
        .map_err(|e| AppError::with_detail("E_PRESET_WRITE", "프리셋을 저장하지 못했어요.", e))?;
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// id로 프리셋 찾기.
pub fn find_preset(file: &PresetFile, id: &str) -> Result<Preset, AppError> {
    file.presets
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| AppError::with_detail("E_PRESET_NOT_FOUND", "프리셋을 찾을 수 없어요.", id))
}

/// 저장: 기존 항목이 있으면 현재 상태를 `history`에 스냅샷으로 남기고
/// `version`을 올린 뒤 새 필드 값으로 교체한다. 없으면 version=1로 새로 만든다.
/// 클라이언트가 보낸 `version`/`history`는 무시하고 서버가 다시 계산한다
/// (복원도 이 함수 하나로 처리 — 옛 필드 값을 넣고 저장하면 현재 버전이
/// history에 보존되면서 새 버전 번호로 복원됨).
pub fn upsert_preset(
    data_root: &Path,
    incoming: Preset,
    saved_at: i64,
) -> Result<Preset, AppError> {
    let mut file = load_presets(data_root)?;
    let saved = match file.presets.iter().position(|p| p.id == incoming.id) {
        Some(idx) => {
            let existing = file.presets[idx].clone();
            let snapshot = PresetSnapshot {
                version: existing.version,
                label: existing.label,
                success_criteria: existing.success_criteria,
                prefix: existing.prefix,
                suffix: existing.suffix,
                negative: existing.negative,
                params: existing.params,
                saved_at,
            };
            let mut history = existing.history;
            history.insert(0, snapshot);
            let next = Preset {
                id: incoming.id,
                label: incoming.label,
                version: existing.version + 1,
                history,
                success_criteria: incoming.success_criteria,
                prefix: incoming.prefix,
                suffix: incoming.suffix,
                negative: incoming.negative,
                params: incoming.params,
            };
            file.presets[idx] = next.clone();
            next
        }
        None => {
            let next = Preset {
                version: 1,
                history: Vec::new(),
                ..incoming
            };
            file.presets.push(next.clone());
            next
        }
    };
    save_file(data_root, &file)?;
    Ok(saved)
}

/// 내보내기: 현재 presets.json(또는 내장 기본값)을 사용자가 고른 임의 경로로 복사.
pub fn export_presets(data_root: &Path, dest_path: &Path) -> Result<(), AppError> {
    let file = load_presets(data_root)?;
    let text = serde_json::to_string_pretty(&file)
        .map_err(|e| AppError::with_detail("E_PRESET_WRITE", "프리셋을 저장하지 못했어요.", e))?;
    std::fs::write(dest_path, text)?;
    Ok(())
}

/// 가져오기: schemaVersion을 먼저 검증(불일치 시 아무 것도 바꾸지 않고 에러)한
/// 뒤, 각 프리셋을 `upsert_preset`으로 병합한다 — 기존 상태는 history에
/// 스냅샷으로 보존되며 새 버전으로 저장된다(저장과 동일한 버전 관리 재사용).
pub fn import_presets(
    data_root: &Path,
    src_path: &Path,
    saved_at: i64,
) -> Result<Vec<Preset>, AppError> {
    let text = std::fs::read_to_string(src_path)?;
    let imported: PresetFile = serde_json::from_str(&text)
        .map_err(|e| AppError::with_detail("E_PRESET_PARSE", "가져올 파일을 읽지 못했어요.", e))?;
    if imported.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(AppError::with_detail(
            "E_PRESET_SCHEMA_VERSION",
            "지원하지 않는 프리셋 파일 버전이에요.",
            format!(
                "schemaVersion {} (지원: {SUPPORTED_SCHEMA_VERSION})",
                imported.schema_version
            ),
        ));
    }
    for preset in imported.presets {
        upsert_preset(data_root, preset, saved_at)?;
    }
    Ok(load_presets(data_root)?.presets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_presets_parse_with_six_entries() {
        let file: PresetFile = serde_json::from_str(DEFAULT_PRESETS).unwrap();
        assert_eq!(file.schema_version, 1);
        assert_eq!(file.presets.len(), 6, "T2.2: 기본 프리셋 6종");
        // TAD §3.2 예시 프리셋 존재
        let storybook = file.presets.iter().find(|p| p.id == "storybook").unwrap();
        assert_eq!(storybook.params.width, 1024);
        assert!(!storybook.prefix.is_empty());
        assert!(!storybook.negative.is_empty());
    }

    #[test]
    fn load_falls_back_to_defaults_and_finds_by_id() {
        let dir = tempfile::tempdir().unwrap();
        let file = load_presets(dir.path()).unwrap();
        assert!(find_preset(&file, "storybook").is_ok());
        let err = find_preset(&file, "no-such").unwrap_err();
        assert_eq!(err.code, "E_PRESET_NOT_FOUND");
    }

    #[test]
    fn user_presets_override_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("presets.json"),
            r#"{"schemaVersion":1,"presets":[{"id":"mine","label":{},"version":1,"prefix":"p","suffix":"s","negative":"n","params":{"steps":20,"cfg":6.0,"width":512,"height":512}}]}"#,
        )
        .unwrap();
        let file = load_presets(dir.path()).unwrap();
        assert_eq!(file.presets.len(), 1);
        assert!(find_preset(&file, "mine").is_ok());
    }

    fn edited(id: &str, prefix: &str) -> Preset {
        Preset {
            id: id.to_string(),
            label: serde_json::json!({ "ko": "동화같은", "en": "Storybook" }),
            version: 0, // 서버가 다시 계산 — 무시됨
            history: Vec::new(),
            success_criteria: "파스텔톤 유지".to_string(),
            prefix: prefix.to_string(),
            suffix: "s".to_string(),
            negative: "n".to_string(),
            params: PresetParams {
                steps: 30,
                cfg: 7.0,
                width: 1024,
                height: 1024,
            },
        }
    }

    #[test]
    fn save_bumps_version_and_snapshots_previous_state() {
        let dir = tempfile::tempdir().unwrap();
        let v1 = load_presets(dir.path())
            .unwrap()
            .presets
            .into_iter()
            .find(|p| p.id == "storybook")
            .unwrap();
        assert_eq!(v1.version, 1);

        let saved = upsert_preset(dir.path(), edited("storybook", "v2 prefix"), 1_000).unwrap();
        assert_eq!(saved.version, 2);
        assert_eq!(saved.prefix, "v2 prefix");
        assert_eq!(saved.history.len(), 1);
        assert_eq!(saved.history[0].version, 1);
        assert_eq!(saved.history[0].prefix, v1.prefix);
        assert_eq!(saved.history[0].saved_at, 1_000);

        // 다시 저장하면 최신 스냅샷이 맨 앞에 쌓인다.
        let saved2 = upsert_preset(dir.path(), edited("storybook", "v3 prefix"), 2_000).unwrap();
        assert_eq!(saved2.version, 3);
        assert_eq!(saved2.history.len(), 2);
        assert_eq!(saved2.history[0].version, 2);
        assert_eq!(saved2.history[0].prefix, "v2 prefix");
        assert_eq!(saved2.history[1].version, 1);
    }

    #[test]
    fn save_new_id_starts_at_version_one_with_empty_history() {
        let dir = tempfile::tempdir().unwrap();
        let saved = upsert_preset(dir.path(), edited("custom", "p"), 1_000).unwrap();
        assert_eq!(saved.version, 1);
        assert!(saved.history.is_empty());
    }

    #[test]
    fn restore_is_save_with_snapshot_fields_and_preserves_all_versions() {
        let dir = tempfile::tempdir().unwrap();
        let original_prefix = load_presets(dir.path())
            .unwrap()
            .presets
            .into_iter()
            .find(|p| p.id == "storybook")
            .unwrap()
            .prefix;

        let v2 = upsert_preset(dir.path(), edited("storybook", "v2 prefix"), 1_000).unwrap();
        let snapshot_v1 = v2.history.iter().find(|s| s.version == 1).unwrap().clone();

        // 복원: 스냅샷 필드로 다시 저장 — 새 커맨드 없이 upsert_preset 재사용.
        let mut restore_payload = edited("storybook", &snapshot_v1.prefix);
        restore_payload.label = snapshot_v1.label.clone();
        restore_payload.success_criteria = snapshot_v1.success_criteria.clone();
        restore_payload.suffix = snapshot_v1.suffix.clone();
        restore_payload.negative = snapshot_v1.negative.clone();
        restore_payload.params = snapshot_v1.params.clone();
        let restored = upsert_preset(dir.path(), restore_payload, 3_000).unwrap();

        assert_eq!(restored.version, 3);
        assert_eq!(restored.prefix, original_prefix);
        // v1과 v2 모두 history에 보존됨
        assert_eq!(restored.history.len(), 2);
        assert_eq!(restored.history[0].version, 2);
        assert_eq!(restored.history[0].prefix, "v2 prefix");
        assert_eq!(restored.history[1].version, 1);
    }

    #[test]
    fn saved_presets_persist_across_reload_with_camel_case() {
        let dir = tempfile::tempdir().unwrap();
        upsert_preset(dir.path(), edited("storybook", "v2 prefix"), 1_000).unwrap();
        let text = std::fs::read_to_string(dir.path().join("presets.json")).unwrap();
        assert!(text.contains("successCriteria"));
        assert!(text.contains("savedAt"));
        let reloaded = load_presets(dir.path()).unwrap();
        let storybook = find_preset(&reloaded, "storybook").unwrap();
        assert_eq!(storybook.version, 2);
        assert_eq!(storybook.history.len(), 1);
    }

    #[test]
    fn export_writes_current_presets_to_arbitrary_path() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out/backup.json");
        // 부모 폴더가 없어도 되는지는 export_presets 책임이 아님 — 호출부가 만든다
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        export_presets(dir.path(), &dest).unwrap();

        let exported: PresetFile =
            serde_json::from_str(&std::fs::read_to_string(&dest).unwrap()).unwrap();
        assert_eq!(exported.schema_version, 1);
        assert_eq!(exported.presets.len(), 6);
    }

    #[test]
    fn import_rejects_unsupported_schema_version_without_side_effects() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("in.json");
        std::fs::write(&src, r#"{"schemaVersion":99,"presets":[]}"#).unwrap();

        let err = import_presets(dir.path(), &src, 1_000).unwrap_err();
        assert_eq!(err.code, "E_PRESET_SCHEMA_VERSION");
        // 검증 실패 시 presets.json이 새로 만들어지지 않아야 한다
        assert!(!dir.path().join("presets.json").exists());
    }

    #[test]
    fn import_merges_via_upsert_bumping_version_and_keeping_history() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("in.json");
        std::fs::write(
            &src,
            r#"{"schemaVersion":1,"presets":[
                {"id":"storybook","label":{"ko":"수정됨"},"version":1,"prefix":"imported prefix","suffix":"s","negative":"n","params":{"steps":10,"cfg":5.0,"width":512,"height":512}},
                {"id":"new-one","label":{"ko":"새 프리셋"},"version":1,"prefix":"p","suffix":"s","negative":"n","params":{"steps":10,"cfg":5.0,"width":512,"height":512}}
            ]}"#,
        )
        .unwrap();

        let merged = import_presets(dir.path(), &src, 2_000).unwrap();
        assert_eq!(merged.len(), 7, "기존 6종 + 신규 1종");

        let storybook = merged.iter().find(|p| p.id == "storybook").unwrap();
        assert_eq!(storybook.version, 2, "기존 프리셋은 버전이 올라가야 함");
        assert_eq!(storybook.prefix, "imported prefix");
        assert_eq!(storybook.history.len(), 1, "이전 상태가 history에 보존됨");
        assert_eq!(storybook.history[0].version, 1);

        let new_one = merged.iter().find(|p| p.id == "new-one").unwrap();
        assert_eq!(new_one.version, 1);
        assert!(new_one.history.is_empty());
    }

    #[test]
    fn export_then_import_into_fresh_data_root_keeps_same_preset_ids_and_content() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let dest = dir_a.path().join("export.json");
        export_presets(dir_a.path(), &dest).unwrap();

        // dir_b는 presets.json이 없어 내장 기본값(동일 6종)이 "기존"으로 취급됨 —
        // 가져오기는 저장과 같은 버전 관리를 타므로 내용이 같아도 버전은 올라간다.
        let merged = import_presets(dir_b.path(), &dest, 3_000).unwrap();
        assert_eq!(merged.len(), 6);
        let storybook = merged.iter().find(|p| p.id == "storybook").unwrap();
        assert_eq!(storybook.version, 2);
        assert_eq!(storybook.history.len(), 1);
        assert_eq!(
            storybook.history[0].prefix, storybook.prefix,
            "내용 자체는 그대로"
        );
    }
}
