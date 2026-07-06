//! 생성 오케스트레이션 (TAD §5 generate, §6 연동 흐름).
//!
//! 프리셋+키워드 → 프롬프트 조립(§4) → 슬롯 치환(§6) → POST /prompt →
//! WS 진행률 → 출력 이미지를 outputs/YYYY-MM/로 이동 → DB 기록.
//! tauri 비의존 — command와 dev CLI가 같은 경로를 사용한다.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::db::models::Generation;
use crate::db::Db;
use crate::error::AppError;
use crate::prompt::assemble::{assemble_negative, assemble_prompt, StyleFragments};
use crate::prompt::presets::{find_preset, load_presets};
use crate::prompt::workflow::{apply_slots, build_prompt_payload, WorkflowParams, SDXL_BASE};

use super::client;
use super::fallback::{self, Attempt};

/// 진행 콜백으로 전달되는 갱신 (진행률 또는 사용자 고지 — T1.5 폴백).
#[derive(Debug, Clone)]
pub enum GenUpdate {
    /// 0.0~1.0
    Progress(f64),
    /// 폴백 등 사용자 고지 문구 (04 §6 톤)
    Notice(String),
}

/// generate 입력 (TAD §5).
#[derive(Debug, Clone)]
pub struct GenerateRequest {
    pub preset_id: String,
    pub keyword: String,
    pub count: u32,
    /// (width, height). None이면 프리셋 기본값
    pub size: Option<(u32, u32)>,
    /// None이면 랜덤
    pub seed: Option<i64>,
}

/// gen://progress 페이로드.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenProgress {
    pub job_id: String,
    /// 0.0~1.0
    pub progress: f64,
    /// 폴백 등 사용자 고지 (T1.5, 04 §6 톤)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}

/// gen://done 페이로드.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenDone {
    pub job_id: String,
    /// 생성된 이미지의 DB id 목록
    pub generation_ids: Vec<String>,
    /// 앱 데이터 루트 기준 상대 경로 목록
    pub image_paths: Vec<String>,
}

/// 설치된 체크포인트 해석: SDXL 우선, 없으면 첫 .safetensors (light 프로파일 폴백).
pub fn resolve_checkpoint(data_root: &Path) -> Option<String> {
    let dir = data_root.join("models/checkpoints");
    let entries: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".safetensors"))
        .collect();
    entries
        .iter()
        .find(|n| n.starts_with("sd_xl_base"))
        .or_else(|| entries.first())
        .cloned()
}

/// 랜덤 시드 (미지정 시 — TAD §4).
pub fn random_seed() -> i64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as i64 + d.as_secs() as i64 * 1_000_000_007)
        .unwrap_or(42);
    nanos.abs() % (i32::MAX as i64)
}

/// 출력 저장 폴더: outputs/YYYY-MM (TAD §3). 반환은 루트 기준 상대 경로.
pub fn output_month_dir(now_ms: i64) -> String {
    use chrono::{TimeZone, Utc};
    let dt = Utc
        .timestamp_millis_opt(now_ms)
        .single()
        .unwrap_or_else(|| Utc.timestamp_millis_opt(0).single().unwrap_or_default());
    format!("outputs/{}", dt.format("%Y-%m"))
}

/// 재시도 전 엔진 회복 대기 (프로세스 사망 → T1.2 자동 재시작 창구).
async fn wait_engine_ready(http: &reqwest::Client, base_url: &str, max_secs: u64) {
    for _ in 0..max_secs {
        let ok = http
            .get(format!("{base_url}/system_stats"))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        if ok {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

/// 완성 흐름 실행. 성공 시 GenDone 반환.
/// OOM/프로세스 사망 시 ① 해상도 하향 ② SD1.5 폴백 순으로 자동 재시도 (TAD §6, T1.5).
#[allow(clippy::too_many_arguments)]
pub async fn run_generation(
    job_id: &str,
    data_root: &Path,
    base_url: &str,
    db: &Db,
    req: &GenerateRequest,
    cancel: &tokio::sync::watch::Receiver<bool>,
    mut on_update: impl FnMut(GenUpdate),
) -> Result<GenDone, AppError> {
    // 1) 프리셋 로드 + 한→영 변환(§4 ①②③, T2.3) + 프롬프트 조립 (§4)
    let presets = load_presets(data_root)?;
    let preset = find_preset(&presets, &req.preset_id)?;
    let translation = crate::prompt::translate::translate_keyword(data_root, &req.keyword).await;
    if let Some(warning) = &translation.warning {
        on_update(GenUpdate::Notice(warning.clone()));
    }
    let prompt = assemble_prompt(
        &preset.prefix,
        &translation.translated,
        &StyleFragments::default(), // 스타일 연동은 T4.3/M6
        &preset.suffix,
    );
    let negative = assemble_negative(&preset.negative, "");
    let (width, height) = req
        .size
        .unwrap_or((preset.params.width, preset.params.height));
    let seed = req.seed.unwrap_or_else(random_seed);
    let http = reqwest::Client::new();

    // 2) 시도 루프: 원본 → (OOM 시) 해상도 하향 → SD1.5 폴백 (T1.5)
    let mut attempt = Attempt {
        width,
        height,
        checkpoint: resolve_checkpoint(data_root),
        stage: 0,
    };
    let (images, attempt) = loop {
        let params = WorkflowParams {
            prompt: prompt.clone(),
            negative: negative.clone(),
            seed,
            steps: preset.params.steps,
            cfg: preset.params.cfg,
            width: attempt.width,
            height: attempt.height,
            batch: req.count.clamp(1, 4),
            checkpoint: attempt.checkpoint.clone(),
            lora: None,
            ipadapter: None,
        };
        let workflow = apply_slots(SDXL_BASE, &params)?;
        let payload = build_prompt_payload(workflow, job_id);

        let result = async {
            let prompt_id = client::post_prompt(&http, base_url, &payload).await?;
            client::track_progress(base_url, job_id, &prompt_id, cancel, |value, max| {
                on_update(GenUpdate::Progress(value as f64 / max as f64));
            })
            .await
        }
        .await;

        match result {
            Ok(images) => break (images, attempt),
            Err(err) => {
                // 취소는 폴백 대상 아님 — 그대로 전파
                if err.code == "E_CANCELED" {
                    return Err(err);
                }
                let sd15 = fallback::resolve_sd15(data_root);
                match fallback::next_attempt(&attempt, &err, sd15.as_deref()) {
                    Some((next, notice)) => {
                        on_update(GenUpdate::Notice(notice));
                        // 프로세스 사망이었다면 자동 재시작(T1.2)이 끝날 때까지 대기
                        wait_engine_ready(&http, base_url, 30).await;
                        attempt = next;
                    }
                    None => return Err(err),
                }
            }
        }
    };

    if images.is_empty() {
        return Err(AppError::new(
            "E_ENGINE_NO_OUTPUT",
            "엔진이 이미지를 만들지 못했어요.",
        ));
    }

    // 4) outputs/YYYY-MM/로 이동 + DB 기록 (§3, §6). 경로는 루트 기준 상대 저장.
    let now_ms = chrono::Utc::now().timestamp_millis();
    let month_rel = output_month_dir(now_ms);
    std::fs::create_dir_all(data_root.join(&month_rel))?;

    let mut ids = Vec::new();
    let mut paths = Vec::new();
    for img in &images {
        // --base-directory 기준 ComfyUI 출력 위치
        let src: PathBuf = {
            let mut p = data_root.join("output");
            if !img.subfolder.is_empty() {
                p = p.join(&img.subfolder);
            }
            p.join(&img.filename)
        };
        let gen_id = uuid::Uuid::new_v4().to_string();
        let rel_path = format!("{month_rel}/{gen_id}.png");
        std::fs::rename(&src, data_root.join(&rel_path)).map_err(|e| {
            AppError::with_detail(
                "E_OUTPUT_MOVE",
                "생성된 이미지를 옮기지 못했어요.",
                format!("{} -> {rel_path}: {e}", src.display()),
            )
        })?;

        let generation = Generation {
            id: gen_id.clone(),
            created_at: now_ms,
            image_path: rel_path.clone(),
            thumb_path: rel_path.clone(), // 썸네일 생성은 갤러리(M3)에서
            keyword_ko: Some(req.keyword.clone()),
            prompt_final: prompt.clone(),
            negative: Some(negative.clone()),
            preset_id: Some(preset.id.clone()),
            preset_version: Some(preset.version as i64),
            style_id: None,
            seed,
            steps: Some(preset.params.steps as i64),
            cfg: Some(preset.params.cfg),
            // 폴백이 있었다면 실제 사용된 값 기록 (T1.5)
            width: Some(attempt.width as i64),
            height: Some(attempt.height as i64),
            model: attempt.checkpoint.clone(),
            favorite: false,
        };
        db.insert_generation(&generation).await?;
        ids.push(gen_id);
        paths.push(rel_path);
    }

    Ok(GenDone {
        job_id: job_id.to_string(),
        generation_ids: ids,
        image_paths: paths,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_dir_is_yyyy_mm_under_outputs() {
        // 2026-07-05 12:00:00 UTC
        assert_eq!(output_month_dir(1_783_339_200_000), "outputs/2026-07");
        // 상대 경로 (TAD §3 / CLAUDE.md: 루트 기준 상대 저장)
        assert!(!output_month_dir(0).starts_with('/'));
    }

    #[test]
    fn random_seed_is_nonnegative_and_varies() {
        let a = random_seed();
        assert!(a >= 0);
    }

    #[test]
    fn checkpoint_resolution_prefers_sdxl() {
        let dir = tempfile::tempdir().unwrap();
        let ckpt = dir.path().join("models/checkpoints");
        std::fs::create_dir_all(&ckpt).unwrap();

        // 없음 → None
        assert_eq!(resolve_checkpoint(dir.path()), None);

        // light만 → 그것 사용 (SD1.5 폴백)
        std::fs::write(ckpt.join("v1-5-pruned-emaonly-fp16.safetensors"), b"x").unwrap();
        assert_eq!(
            resolve_checkpoint(dir.path()),
            Some("v1-5-pruned-emaonly-fp16.safetensors".into())
        );

        // SDXL 있으면 우선
        std::fs::write(ckpt.join("sd_xl_base_1.0.safetensors"), b"x").unwrap();
        assert_eq!(
            resolve_checkpoint(dir.path()),
            Some("sd_xl_base_1.0.safetensors".into())
        );
    }
}
