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
use crate::prompt::workflow::{
    apply_slots, build_prompt_payload, IpAdapterParams, LoraParams, WorkflowParams, SDXL_BASE,
    SDXL_IPADAPTER, SDXL_LORA,
};
use crate::styles::{load_styles as load_styles_file, Style};

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
    /// 에센스/LoRA 스타일 (T4.3)
    pub style_id: Option<String>,
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
    /// 이 배치에 사용된 시드 (시드 고정 재생성 — T2.4, F-1.5)
    pub seed: i64,
    /// 생성 전체 소요 ms (T9.1 계측 — docs/11 §P0.1)
    pub duration_ms: u64,
}

/// 진행 이벤트 코얼레싱 (T9.2, docs/11 §P1.6): 스텝마다 IPC 이벤트를 쏘지 않고
/// 진행률이 1% 이상 변했거나 100ms가 지났을 때만 내보낸다.
/// 고지(notice)·최종값(≥1.0)·force는 항상 통과 — 마지막 상태 유실 금지.
pub struct ProgressCoalescer {
    last_progress: f64,
    last_emit: Option<std::time::Instant>,
}

impl Default for ProgressCoalescer {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressCoalescer {
    const MIN_DELTA: f64 = 0.01;
    const MIN_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

    pub fn new() -> Self {
        Self {
            last_progress: f64::NEG_INFINITY,
            last_emit: None,
        }
    }

    pub fn should_emit(&mut self, progress: f64, force: bool) -> bool {
        let recent = self
            .last_emit
            .is_some_and(|t| t.elapsed() < Self::MIN_INTERVAL);
        if !force && progress < 1.0 && progress - self.last_progress < Self::MIN_DELTA && recent {
            return false;
        }
        self.last_progress = progress;
        self.last_emit = Some(std::time::Instant::now());
        true
    }
}

/// 스타일이 프롬프트에 기여하는 조각 (TAD §4 — 트리거워드·에센스).
pub fn style_fragments(style: Option<&Style>) -> StyleFragments {
    match style {
        Some(s) if s.kind == "essence" => StyleFragments {
            trigger_word: None,
            essence_prompt: s.essence_prompt.clone(),
        },
        Some(s) if s.kind == "lora" => StyleFragments {
            trigger_word: s.trigger_word.clone(),
            essence_prompt: None,
        },
        _ => StyleFragments::default(),
    }
}

/// 이번 시도에 쓸 워크플로 템플릿·주입 파라미터 (T4.3, 순수 — 테스트).
/// IP-Adapter/LoRA는 SDXL 전용이라 SD1.5 폴백(stage 2)에서는 base로 떼고
/// 사용자 고지 문구를 함께 돌려준다. 에센스 프롬프트(텍스트)는 유지된다.
#[allow(clippy::type_complexity)]
pub fn plan_style_for_attempt(
    style: Option<&Style>,
    ip_image: Option<&str>,
    sd15_fallback: bool,
) -> (
    &'static str,
    Option<IpAdapterParams>,
    Option<LoraParams>,
    Option<String>,
) {
    let Some(style) = style else {
        return (SDXL_BASE, None, None, None);
    };
    if sd15_fallback {
        return (
            SDXL_BASE,
            None,
            None,
            Some("가벼운 모델에서는 스타일 참조를 뺀 채 생성했어요.".to_string()),
        );
    }
    match style.kind.as_str() {
        "essence" => match ip_image {
            Some(image) => (
                SDXL_IPADAPTER,
                Some(IpAdapterParams {
                    image: image.to_string(),
                    weight: style.ip_adapter_weight.unwrap_or(0.6),
                }),
                None,
                None,
            ),
            // 참조 이미지가 없으면 에센스 프롬프트만으로 (base)
            None => (SDXL_BASE, None, None, None),
        },
        "lora" => match &style.lora_path {
            Some(path) => {
                // WorkflowParams는 models/loras/ 기준 파일명을 받는다 (TAD §6)
                let lora_name = path.rsplit('/').next().unwrap_or(path.as_str()).to_string();
                (
                    SDXL_LORA,
                    None,
                    Some(LoraParams {
                        lora_name,
                        weight: style.lora_weight.unwrap_or(0.8),
                    }),
                    None,
                )
            }
            None => (SDXL_BASE, None, None, None),
        },
        _ => (SDXL_BASE, None, None, None),
    }
}

/// 에센스 참조 이미지 1장을 ComfyUI input 폴더로 복사하고 파일명을 돌려준다.
/// (--base-directory 기준 input/ — LoadImage 노드가 여기서 읽는다)
pub fn prepare_ip_reference(data_root: &Path, style: &Style) -> Option<String> {
    let rel = style.reference_images.first()?;
    let src = data_root.join(rel);
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png");
    let name = format!("style-{}.{ext}", style.id);
    let input_dir = data_root.join("input");
    std::fs::create_dir_all(&input_dir).ok()?;
    std::fs::copy(&src, input_dir.join(&name)).ok()?;
    Some(name)
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
    let mut timer = crate::perf::StageTimer::new();
    // 1) 프리셋·스타일 로드 + 한→영 변환(§4 ①②③, T2.3) + 프롬프트 조립 (§4)
    let presets = load_presets(data_root)?;
    let preset = find_preset(&presets, &req.preset_id)?;
    let style: Option<Style> = match req.style_id.as_deref() {
        Some(style_id) => Some(
            load_styles_file(data_root)?
                .styles
                .into_iter()
                .find(|s| s.id == style_id)
                .ok_or_else(|| {
                    AppError::new("E_STYLE_NOT_FOUND", "선택한 스타일을 찾을 수 없어요.")
                })?,
        ),
        None => None,
    };
    // 에센스 참조 이미지를 엔진 input 폴더로 준비 (T4.3)
    let ip_image = style
        .as_ref()
        .filter(|s| s.kind == "essence")
        .and_then(|s| prepare_ip_reference(data_root, s));

    let translation = crate::prompt::translate::translate_keyword(data_root, &req.keyword).await;
    if let Some(warning) = &translation.warning {
        on_update(GenUpdate::Notice(warning.clone()));
    }
    let prompt = assemble_prompt(
        &preset.prefix,
        &translation.translated,
        &style_fragments(style.as_ref()),
        &preset.suffix,
    );
    let negative = assemble_negative(
        &preset.negative,
        &crate::settings::AppSettings::load(data_root).safe_negative,
    );
    let (width, height) = req
        .size
        .unwrap_or((preset.params.width, preset.params.height));
    let seed = req.seed.unwrap_or_else(random_seed);
    let http = crate::engine::shared_http();
    timer.mark("prepare");

    // 2) 시도 루프: 원본 → (OOM 시) 해상도 하향 → SD1.5 폴백 (T1.5)
    let mut attempts = 0u32;
    let mut attempt = Attempt {
        width,
        height,
        checkpoint: resolve_checkpoint(data_root),
        stage: 0,
    };
    let mut style_drop_notified = false;
    let (images, attempt) = loop {
        // 스타일 경로 (T4.3): SD1.5 폴백(stage 2)에서는 SDXL 전용 어댑터를 뗀다
        let (template, ipadapter, lora, drop_notice) =
            plan_style_for_attempt(style.as_ref(), ip_image.as_deref(), attempt.stage >= 2);
        if let Some(notice) = drop_notice {
            if !style_drop_notified {
                style_drop_notified = true;
                on_update(GenUpdate::Notice(notice));
            }
        }
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
            lora,
            ipadapter,
        };
        let workflow = apply_slots(template, &params)?;
        let payload = build_prompt_payload(workflow, job_id);
        attempts += 1;

        let result = async {
            let prompt_id = client::post_prompt(http, base_url, &payload).await?;
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
                        wait_engine_ready(http, base_url, 30).await;
                        attempt = next;
                    }
                    None => return Err(err),
                }
            }
        }
    };

    timer.mark("engine");

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
            style_id: req.style_id.clone(),
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
    timer.mark("persist");

    // 단계별 소요를 로컬 perf.log에 기록 (T9.1 — 내용(프롬프트)은 남기지 않음)
    crate::perf::append_perf_line(
        data_root,
        &timer.to_log_value(
            "generate",
            serde_json::json!({
                "jobId": job_id,
                "width": attempt.width,
                "height": attempt.height,
                "batch": req.count.clamp(1, 4),
                "steps": preset.params.steps,
                "attempts": attempts,
            }),
        ),
    );

    Ok(GenDone {
        job_id: job_id.to_string(),
        generation_ids: ids,
        image_paths: paths,
        seed,
        duration_ms: timer.total_ms() as u64,
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
    fn coalescer_drops_tiny_rapid_updates_but_keeps_key_events() {
        let mut c = ProgressCoalescer::new();
        // 첫 이벤트는 항상 통과
        assert!(c.should_emit(0.0, false));
        // 직후 미세 진행 → 억제 (28스텝이 스텝마다 이벤트를 쏘던 문제)
        assert!(!c.should_emit(0.005, false));
        assert!(!c.should_emit(0.009, false));
        // 1% 이상 변화 → 통과
        assert!(c.should_emit(0.02, false));
        // 고지(force)는 즉시 통과
        assert!(c.should_emit(0.021, true));
        // 최종값은 억제 금지 — 진행 바가 100%로 끝나야 함
        assert!(c.should_emit(1.0, false));
    }

    fn essence_style() -> crate::styles::Style {
        crate::styles::Style {
            id: "s1".to_string(),
            name: "브랜드".to_string(),
            kind: "essence".to_string(),
            essence_prompt: Some("flat color, simple background".to_string()),
            reference_images: vec!["styles/s1/ref-0.png".to_string()],
            ip_adapter_weight: Some(0.7),
            lora_path: None,
            lora_weight: None,
            trigger_word: None,
            thumb: None,
            created_at: 0,
        }
    }

    #[test]
    fn style_plan_selects_templates_and_params() {
        use crate::prompt::workflow::{SDXL_BASE, SDXL_IPADAPTER, SDXL_LORA};

        // 스타일 없음 → base
        let (t, ip, lora, notice) = plan_style_for_attempt(None, None, false);
        assert_eq!(t, SDXL_BASE);
        assert!(ip.is_none() && lora.is_none() && notice.is_none());

        // 에센스 + 참조 이미지 → ipadapter 템플릿 + weight 반영
        let style = essence_style();
        let (t, ip, _, _) = plan_style_for_attempt(Some(&style), Some("style-s1.png"), false);
        assert_eq!(t, SDXL_IPADAPTER);
        let ip = ip.unwrap();
        assert_eq!(ip.image, "style-s1.png");
        assert_eq!(ip.weight, 0.7);

        // 에센스인데 참조 이미지가 준비 안 됨 → base (에센스 프롬프트만)
        let (t, ip, _, _) = plan_style_for_attempt(Some(&style), None, false);
        assert_eq!(t, SDXL_BASE);
        assert!(ip.is_none());

        // SD1.5 폴백 → base + 고지 (IP-Adapter는 SDXL 전용)
        let (t, ip, _, notice) = plan_style_for_attempt(Some(&style), Some("x.png"), true);
        assert_eq!(t, SDXL_BASE);
        assert!(ip.is_none());
        assert!(notice.unwrap().contains("스타일 참조"));

        // LoRA 스타일 → lora 템플릿 + 파일명·weight
        let mut lora_style = essence_style();
        lora_style.kind = "lora".to_string();
        lora_style.lora_path = Some("models/loras/brand_v1.safetensors".to_string());
        lora_style.lora_weight = Some(0.9);
        let (t, _, lora, _) = plan_style_for_attempt(Some(&lora_style), None, false);
        assert_eq!(t, SDXL_LORA);
        let lora = lora.unwrap();
        assert_eq!(lora.lora_name, "brand_v1.safetensors");
        assert_eq!(lora.weight, 0.9);
    }

    #[test]
    fn style_fragments_by_kind() {
        let essence = essence_style();
        let frags = style_fragments(Some(&essence));
        assert_eq!(
            frags.essence_prompt.as_deref(),
            Some("flat color, simple background")
        );
        assert!(frags.trigger_word.is_none());

        let mut lora = essence_style();
        lora.kind = "lora".to_string();
        lora.trigger_word = Some("brandstyle".to_string());
        let frags = style_fragments(Some(&lora));
        assert_eq!(frags.trigger_word.as_deref(), Some("brandstyle"));
        assert!(frags.essence_prompt.is_none());

        assert!(style_fragments(None).essence_prompt.is_none());
    }

    #[test]
    fn ip_reference_is_copied_into_input_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("styles/s1")).unwrap();
        std::fs::write(dir.path().join("styles/s1/ref-0.png"), b"img").unwrap();

        let name = prepare_ip_reference(dir.path(), &essence_style()).unwrap();
        assert_eq!(name, "style-s1.png");
        assert!(dir.path().join("input/style-s1.png").exists());

        // 참조 이미지가 없는 스타일 → None
        let mut empty = essence_style();
        empty.reference_images.clear();
        assert!(prepare_ip_reference(dir.path(), &empty).is_none());
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
