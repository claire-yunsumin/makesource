//! kohya 학습 잡 러너 (TAD §8, T6.3).
//!
//! 흐름: 평평한 데이터셋(`datasets/{id}/`, D-009)을 kohya 규약
//! `training/{jobId}/img/{repeats}_{trigger}/`로 복사 → venv 파이썬으로
//! kohya 학습 스크립트 실행(설치된 체크포인트에 맞춰 SDXL/SD1.5 스크립트
//! 선택) → stderr/stdout을 라인 버퍼로 모아 parser로 해석해 진행/샘플
//! 업데이트 콜백 → 완료 시 .safetensors를 `models/loras/`로 이동(TAD §8 5;
//! styles.json 등록은 T6.4). 취소는 watch 채널 → 프로세스 kill.

use std::path::{Path, PathBuf};

use crate::bootstrap::kohya;
use crate::engine::generation::resolve_checkpoint;
use crate::error::AppError;
use crate::training::parser::{parse_line, TrainEvent};
use crate::training::profiles::Profile;

/// 러너가 밖으로 알리는 업데이트 (command 계층이 train:// 이벤트로 변환).
#[derive(Debug, Clone, PartialEq)]
pub enum TrainUpdate {
    Progress {
        /// 0.0 ~ 1.0
        progress: f64,
        eta_seconds: Option<i64>,
        loss: Option<f64>,
        /// (현재, 전체) — epoch 경계에서만 Some
        epoch: Option<(u32, u32)>,
    },
    /// epoch 샘플 이미지 (데이터 루트 기준 상대 경로 아님 — 절대 경로)
    Sample { image_path: String },
}

/// 작업 폴더 배치 결과.
#[derive(Debug, Clone, PartialEq)]
pub struct KohyaLayout {
    pub work_dir: PathBuf,
    pub img_dir: PathBuf,
    pub output_dir: PathBuf,
    pub sample_dir: PathBuf,
    pub sample_prompts: PathBuf,
    pub image_count: usize,
}

/// 트리거 단어를 폴더 이름에 쓸 수 있게 정리 (영숫자만, 소문자, 비면 "style").
pub fn sanitize_trigger(trigger: &str) -> String {
    let cleaned: String = trigger
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    if cleaned.is_empty() {
        "style".to_string()
    } else {
        cleaned
    }
}

fn is_image_ext(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "png" | "jpg" | "jpeg" | "webp"
    )
}

/// 데이터셋 폴더의 이미지 수 (training_start의 저렴한 사전 검증용 —
/// 복사는 spawn된 태스크에서, CLAUDE.md 규칙 4).
pub fn count_dataset_images(dataset_dir: &Path) -> Result<usize, AppError> {
    if !dataset_dir.is_dir() {
        return Err(AppError::with_detail(
            "E_DATASET_NOT_FOUND",
            "학습 데이터셋 폴더를 찾을 수 없어요.",
            dataset_dir.display(),
        ));
    }
    let count = std::fs::read_dir(dataset_dir)?
        .flatten()
        .filter(|e| e.path().is_file() && is_image_ext(&e.path()))
        .count();
    if count == 0 {
        return Err(AppError::new(
            "E_DATASET_EMPTY",
            "데이터셋에 이미지가 없어요. 이미지를 추가한 뒤 다시 시도해 주세요.",
        ));
    }
    Ok(count)
}

/// D-009: 평평한 데이터셋을 kohya 규약 폴더로 복사 (이미지 + {basename}.txt
/// 캡션). 샘플 프롬프트 파일(--sample_prompts 필수 — 없으면 kohya가 샘플을
/// 아예 만들지 않음)도 트리거 단어로 생성한다.
pub fn prepare_kohya_layout(
    data_root: &Path,
    job_id: &str,
    dataset_dir: &Path,
    profile: &Profile,
    trigger: &str,
) -> Result<KohyaLayout, AppError> {
    count_dataset_images(dataset_dir)?;
    let work_dir = data_root.join("training").join(job_id);
    let img_dir =
        work_dir
            .join("img")
            .join(format!("{}_{}", profile.repeats, sanitize_trigger(trigger)));
    let output_dir = work_dir.join("output");
    let sample_dir = output_dir.join("sample");
    std::fs::create_dir_all(&img_dir)?;
    std::fs::create_dir_all(&sample_dir)?;

    let mut image_count = 0usize;
    for entry in std::fs::read_dir(dataset_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_image = is_image_ext(&path);
        let is_caption = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("txt"));
        if !is_image && !is_caption {
            continue;
        }
        if let Some(name) = path.file_name() {
            std::fs::copy(&path, img_dir.join(name))?;
        }
        if is_image {
            image_count += 1;
        }
    }

    // kohya 샘플 프롬프트 파일 (한 줄 = 프롬프트 + 크기/스텝 플래그)
    let sample_prompts = work_dir.join("sample_prompts.txt");
    std::fs::write(
        &sample_prompts,
        format!(
            "{}, sample illustration --w {res} --h {res} --s 20\n",
            sanitize_trigger(trigger),
            res = profile.resolution.min(1024),
        ),
    )?;

    Ok(KohyaLayout {
        work_dir,
        img_dir,
        output_dir,
        sample_dir,
        sample_prompts,
        image_count,
    })
}

/// 설치된 체크포인트 해석 (engine::generation::resolve_checkpoint 재사용 —
/// SDXL 우선, 없으면 폴백 체크포인트). 반환: (경로, SDXL 여부).
pub fn resolve_base_model(data_root: &Path) -> Result<(PathBuf, bool), AppError> {
    let name = resolve_checkpoint(data_root).ok_or_else(|| {
        AppError::new(
            "E_BASE_MODEL_MISSING",
            "기반 모델이 없어요. 처음 사용 설정(모델 다운로드)을 마치면 학습할 수 있어요.",
        )
    })?;
    let is_sdxl = name.starts_with("sd_xl");
    Ok((data_root.join("models/checkpoints").join(name), is_sdxl))
}

/// 모델 계열에 맞는 kohya 학습 스크립트 파일명.
pub fn training_script(is_sdxl: bool) -> &'static str {
    if is_sdxl {
        "sdxl_train_network.py"
    } else {
        "train_network.py"
    }
}

/// kohya 학습 스크립트 인자 조립 (순수 — 테스트 대상).
pub fn build_kohya_args(
    layout: &KohyaLayout,
    profile: &Profile,
    base_model: &Path,
    output_name: &str,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "--pretrained_model_name_or_path".into(),
        base_model.to_string_lossy().into_owned(),
        "--train_data_dir".into(),
        layout
            .img_dir
            .parent()
            .unwrap_or(&layout.img_dir)
            .to_string_lossy()
            .into_owned(),
        "--output_dir".into(),
        layout.output_dir.to_string_lossy().into_owned(),
        "--output_name".into(),
        output_name.into(),
        "--network_module".into(),
        "networks.lora".into(),
        "--save_model_as".into(),
        "safetensors".into(),
        // kohya 기본 캡션 확장자는 .caption — T6.2가 저장한 .txt를 읽게 지정
        "--caption_extension".into(),
        ".txt".into(),
        // 샘플 프롬프트 없이는 sample_every_n_epochs가 있어도 샘플 생성 안 됨
        "--sample_prompts".into(),
        layout.sample_prompts.to_string_lossy().into_owned(),
        "--mixed_precision".into(),
        "no".into(), // MPS는 fp16 mixed에서 불안정 — 안전 기본값
    ];
    for (key, value) in [
        ("--max_train_epochs", profile.max_train_epochs.to_string()),
        ("--network_dim", profile.network_dim.to_string()),
        ("--network_alpha", profile.network_alpha.to_string()),
        ("--learning_rate", format!("{}", profile.learning_rate)),
        ("--resolution", format!("{0},{0}", profile.resolution)),
        ("--train_batch_size", profile.train_batch_size.to_string()),
        (
            "--sample_every_n_epochs",
            profile.sample_every_n_epochs.to_string(),
        ),
    ] {
        args.push(key.into());
        args.push(value);
    }
    args
}

/// 최종 산출물(.safetensors)을 models/loras/로 이동하고 루트 기준 상대 경로
/// 반환. 같은 이름이 이미 있으면 -2, -3…으로 비켜 간다(기존 LoRA를 덮어쓰면
/// 그 파일을 쓰는 다른 스타일이 조용히 바뀜).
pub fn collect_lora_artifact(
    data_root: &Path,
    layout: &KohyaLayout,
    output_name: &str,
) -> Result<String, AppError> {
    let src = layout.output_dir.join(format!("{output_name}.safetensors"));
    if !src.exists() {
        return Err(AppError::with_detail(
            "E_TRAIN_NO_ARTIFACT",
            "학습이 끝났지만 결과 파일을 찾지 못했어요.",
            src.display(),
        ));
    }
    let loras_dir = data_root.join("models/loras");
    std::fs::create_dir_all(&loras_dir)?;
    let dest = crate::commands::export::unique_path(&loras_dir, output_name, "safetensors");
    std::fs::rename(&src, &dest).or_else(|_| {
        // 볼륨 경계 등으로 rename 실패 시 복사 폴백
        std::fs::copy(&src, &dest).map(|_| ())
    })?;
    let file_name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("{output_name}.safetensors"));
    Ok(format!("models/loras/{file_name}"))
}

/// 청크 스트림 → 완전한 논리 줄. tqdm은 \r로 같은 줄을 덮어쓰고, 4KB read
/// 경계가 줄이나 멀티바이트 문자 한가운데에 떨어질 수 있어 바이트 단위로
/// 버퍼링한다 — 잘린 조각을 완전한 줄로 오인하면 진행률이 역행한다.
#[derive(Default)]
pub struct LineBuffer {
    pending: Vec<u8>,
}

impl LineBuffer {
    /// 청크를 붙이고, 마지막 \r/\n까지의 완전한 줄들만 돌려준다.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.pending.extend_from_slice(chunk);
        let cut = match self.pending.iter().rposition(|&b| b == b'\n' || b == b'\r') {
            Some(idx) => idx + 1,
            None => return Vec::new(),
        };
        let complete: Vec<u8> = self.pending.drain(..cut).collect();
        String::from_utf8_lossy(&complete)
            .split(['\r', '\n'])
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// EOF: 남은 조각을 마지막 줄로 반환.
    pub fn flush(&mut self) -> Option<String> {
        if self.pending.is_empty() {
            return None;
        }
        let rest = std::mem::take(&mut self.pending);
        let line = String::from_utf8_lossy(&rest).trim().to_string();
        (!line.is_empty()).then_some(line)
    }
}

/// 파싱된 줄 → 업데이트 방출 + epoch 추적. (루프와 종료 드레인 양쪽에서 사용)
fn dispatch_line(
    line: &str,
    last_epoch: &mut Option<(u32, u32)>,
    on_update: &mut impl FnMut(TrainUpdate),
) {
    match parse_line(line) {
        Some(TrainEvent::Progress {
            step,
            total,
            loss,
            eta_seconds,
        }) => on_update(TrainUpdate::Progress {
            progress: f64::from(step) / f64::from(total.max(1)),
            eta_seconds: eta_seconds.map(|s| s as i64),
            loss,
            epoch: *last_epoch,
        }),
        Some(TrainEvent::Epoch { current, total }) => {
            *last_epoch = Some((current, total));
        }
        None => {}
    }
}

/// 새 샘플 PNG 스캔 → Sample 업데이트 방출.
fn scan_samples(
    sample_dir: &Path,
    seen: &mut std::collections::HashSet<PathBuf>,
    on_update: &mut impl FnMut(TrainUpdate),
) {
    if let Ok(entries) = std::fs::read_dir(sample_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_png = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("png"));
            if is_png && seen.insert(path.clone()) {
                on_update(TrainUpdate::Sample {
                    image_path: path.to_string_lossy().into_owned(),
                });
            }
        }
    }
}

/// 학습 실행. 진행/샘플은 on_update로, 완료 시 LoRA 상대 경로 반환.
/// cancel_rx가 true가 되면 프로세스를 종료하고 E_CANCELED를 돌려준다.
pub async fn run_training(
    data_root: &Path,
    layout: &KohyaLayout,
    profile: &Profile,
    output_name: &str,
    cancel_rx: &tokio::sync::watch::Receiver<bool>,
    mut on_update: impl FnMut(TrainUpdate),
) -> Result<String, AppError> {
    use tokio::io::AsyncReadExt;

    let python = data_root.join("runtime/venv/bin/python");
    let (base_model, is_sdxl) = resolve_base_model(data_root)?;
    let script = kohya::kohya_dir(data_root).join(training_script(is_sdxl));
    if !python.exists() || !script.exists() {
        return Err(AppError::new(
            "E_KOHYA_NOT_INSTALLED",
            "학습 도구가 아직 설치되지 않았어요. 학습 시작 전에 설치를 마쳐 주세요.",
        ));
    }

    let args = build_kohya_args(layout, profile, &base_model, output_name);
    let log_path = layout.work_dir.join("train.log");
    let mut log_file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(f) => Some(f),
        Err(e) => {
            // 로그 없이도 학습은 계속 — 단, 조용히 삼키지 않는다
            eprintln!("train.log 열기 실패 ({}): {e}", log_path.display());
            None
        }
    };

    let mut child = tokio::process::Command::new(&python)
        .arg(&script)
        .args(&args)
        .current_dir(kohya::kohya_dir(data_root))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::with_detail("E_TRAIN_SPAWN", "학습을 시작하지 못했어요.", e))?;

    // stdout/stderr 모두에서 진행줄이 나올 수 있다 (tqdm은 stderr).
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let mut out_lines = LineBuffer::default();
    let mut err_lines = LineBuffer::default();
    let mut out_buf = [0u8; 4096];
    let mut err_buf = [0u8; 4096];
    let mut cancel = cancel_rx.clone();
    let mut last_epoch: Option<(u32, u32)> = None;
    let mut seen_samples: std::collections::HashSet<PathBuf> = Default::default();
    // 샘플 폴더 스캔은 epoch 경계에만 새 파일이 생기므로 시간 스로틀
    let mut last_scan = std::time::Instant::now();
    const SCAN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);

    let write_log = |log_file: &mut Option<std::fs::File>, chunk: &[u8]| {
        if let Some(f) = log_file {
            use std::io::Write;
            let _ = f.write_all(chunk);
        }
    };

    let status = loop {
        tokio::select! {
            // 취소 요청 → 프로세스 종료
            changed = cancel.changed() => {
                if changed.is_ok() && *cancel.borrow() {
                    let _ = child.kill().await;
                    return Err(AppError::new("E_CANCELED", "학습을 취소했어요."));
                }
            }
            read = async {
                match stdout.as_mut() {
                    Some(out) => out.read(&mut out_buf).await,
                    None => std::future::pending().await,
                }
            } => {
                match read {
                    Ok(0) | Err(_) => { stdout = None; }
                    Ok(n) => {
                        write_log(&mut log_file, &out_buf[..n]);
                        for line in out_lines.push(&out_buf[..n]) {
                            dispatch_line(&line, &mut last_epoch, &mut on_update);
                        }
                    }
                }
            }
            read = async {
                match stderr.as_mut() {
                    Some(err) => err.read(&mut err_buf).await,
                    None => std::future::pending().await,
                }
            } => {
                match read {
                    Ok(0) | Err(_) => { stderr = None; }
                    Ok(n) => {
                        write_log(&mut log_file, &err_buf[..n]);
                        for line in err_lines.push(&err_buf[..n]) {
                            dispatch_line(&line, &mut last_epoch, &mut on_update);
                        }
                    }
                }
            }
            exit = child.wait() => {
                break exit.map_err(|e| {
                    AppError::with_detail("E_TRAIN_SPAWN", "학습 프로세스 오류예요.", e)
                })?;
            }
        }

        if last_scan.elapsed() >= SCAN_INTERVAL {
            last_scan = std::time::Instant::now();
            scan_samples(&layout.sample_dir, &mut seen_samples, &mut on_update);
        }
    };

    // 종료 드레인: wait()가 먼저 이기면 파이프 버퍼에 남은 마지막 출력
    // (최종 100% 진행줄, 마지막 epoch 샘플)이 유실된다 — EOF까지 마저 읽는다.
    if let Some(mut out) = stdout.take() {
        let mut rest = Vec::new();
        if out.read_to_end(&mut rest).await.is_ok() && !rest.is_empty() {
            write_log(&mut log_file, &rest);
            for line in out_lines.push(&rest) {
                dispatch_line(&line, &mut last_epoch, &mut on_update);
            }
        }
    }
    if let Some(mut err) = stderr.take() {
        let mut rest = Vec::new();
        if err.read_to_end(&mut rest).await.is_ok() && !rest.is_empty() {
            write_log(&mut log_file, &rest);
            for line in err_lines.push(&rest) {
                dispatch_line(&line, &mut last_epoch, &mut on_update);
            }
        }
    }
    for lines in [&mut out_lines, &mut err_lines] {
        if let Some(line) = lines.flush() {
            dispatch_line(&line, &mut last_epoch, &mut on_update);
        }
    }
    scan_samples(&layout.sample_dir, &mut seen_samples, &mut on_update);

    if !status.success() {
        return Err(AppError::with_detail(
            "E_TRAIN_FAILED",
            "학습이 실패했어요. 로그를 확인해 주세요.",
            format!("exit: {status}, log: {}", log_path.display()),
        ));
    }
    collect_lora_artifact(data_root, layout, output_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::training::profiles::{load_profile, ProfileKind};

    fn make_dataset(dir: &Path, images: usize) {
        std::fs::create_dir_all(dir).unwrap();
        for i in 0..images {
            std::fs::write(dir.join(format!("img-{i:03}.png")), b"img").unwrap();
            std::fs::write(dir.join(format!("img-{i:03}.txt")), b"caption").unwrap();
        }
    }

    fn fast() -> Profile {
        load_profile(ProfileKind::Fast).unwrap()
    }

    #[test]
    fn sanitize_trigger_keeps_alnum_lowercase_with_fallback() {
        assert_eq!(sanitize_trigger("MyStyle"), "mystyle");
        assert_eq!(sanitize_trigger("우리 브랜드!"), "style");
        assert_eq!(sanitize_trigger("brand-2 tone"), "brand2tone");
    }

    #[test]
    fn line_buffer_holds_partial_lines_across_chunk_boundaries() {
        let mut buf = LineBuffer::default();
        // "300/1000" 줄이 청크 경계에서 잘려도 완전한 줄로 오인하지 않는다
        assert!(buf.push(b"steps:  30%|###| 3").is_empty());
        let lines = buf.push(b"00/1000 [00:12<00:28, 24.3it/s]\n");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("300/1000"), "{lines:?}");
        // \r(tqdm 덮어쓰기)도 줄 경계로 취급
        let lines = buf.push(b"steps: 1/10 [00:01<00:09]\rsteps: 2/10 [00:02<00:08]\r");
        assert_eq!(lines.len(), 2);
        assert!(buf.flush().is_none());
        // EOF flush: 개행 없이 끝난 마지막 줄
        assert!(buf.push(b"epoch 2/4").is_empty());
        assert_eq!(buf.flush().as_deref(), Some("epoch 2/4"));
    }

    #[test]
    fn prepare_layout_copies_dataset_and_writes_sample_prompts() {
        let root = tempfile::tempdir().unwrap();
        let dataset = root.path().join("datasets/ds1");
        make_dataset(&dataset, 3);
        std::fs::write(dataset.join("notes.md"), b"skip me").unwrap();

        let layout =
            prepare_kohya_layout(root.path(), "job1", &dataset, &fast(), "MyStyle").unwrap();
        assert_eq!(layout.image_count, 3);
        assert!(layout.img_dir.ends_with("img/10_mystyle"));
        assert!(layout.img_dir.join("img-000.png").exists());
        assert!(layout.img_dir.join("img-000.txt").exists());
        assert!(!layout.img_dir.join("notes.md").exists());
        assert!(layout.sample_dir.is_dir());
        // 샘플 프롬프트 파일 — 없으면 kohya가 샘플을 아예 생성하지 않음
        let prompts = std::fs::read_to_string(&layout.sample_prompts).unwrap();
        assert!(prompts.starts_with("mystyle"), "{prompts}");
        assert!(prompts.contains("--w 1024"));
    }

    #[test]
    fn dataset_validation_rejects_missing_or_empty() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("nope");
        assert_eq!(
            count_dataset_images(&missing).unwrap_err().code,
            "E_DATASET_NOT_FOUND"
        );
        let empty = root.path().join("datasets/empty");
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::write(empty.join("only.txt"), b"caption only").unwrap();
        assert_eq!(
            count_dataset_images(&empty).unwrap_err().code,
            "E_DATASET_EMPTY"
        );
    }

    #[test]
    fn kohya_args_map_profile_layout_captions_and_samples() {
        let root = tempfile::tempdir().unwrap();
        let dataset = root.path().join("datasets/ds1");
        make_dataset(&dataset, 1);
        let layout = prepare_kohya_layout(root.path(), "job1", &dataset, &fast(), "brand").unwrap();
        let args = build_kohya_args(
            &layout,
            &fast(),
            Path::new("/m/sdxl.safetensors"),
            "brand-job1",
        );

        let find = |key: &str| -> &str {
            let idx = args
                .iter()
                .position(|a| a == key)
                .unwrap_or_else(|| panic!("{key} 없음"));
            &args[idx + 1]
        };
        assert_eq!(
            find("--pretrained_model_name_or_path"),
            "/m/sdxl.safetensors"
        );
        // train_data_dir는 img/ 상위 폴더 (kohya가 하위 {repeats}_{trigger}를 읽음)
        assert!(find("--train_data_dir").ends_with("/img"));
        assert_eq!(find("--max_train_epochs"), "4");
        assert_eq!(find("--network_dim"), "8");
        assert_eq!(find("--resolution"), "1024,1024");
        assert_eq!(find("--output_name"), "brand-job1");
        // kohya 기본 캡션 확장자는 .caption — .txt 명시 필수 (T6.2 캡션)
        assert_eq!(find("--caption_extension"), ".txt");
        // 샘플 프롬프트 없으면 sample_every_n_epochs가 있어도 샘플 0장
        assert!(find("--sample_prompts").ends_with("sample_prompts.txt"));
    }

    #[test]
    fn base_model_resolution_prefers_sdxl_and_falls_back_to_sd15() {
        let root = tempfile::tempdir().unwrap();
        let ckpt = root.path().join("models/checkpoints");
        std::fs::create_dir_all(&ckpt).unwrap();
        assert_eq!(
            resolve_base_model(root.path()).unwrap_err().code,
            "E_BASE_MODEL_MISSING"
        );

        // SD1.5만 있는 light 프로파일 기기 — 폴백 + SD1.5 스크립트
        std::fs::write(ckpt.join("v1-5-pruned.safetensors"), b"m").unwrap();
        let (path, is_sdxl) = resolve_base_model(root.path()).unwrap();
        assert!(path.ends_with("v1-5-pruned.safetensors"));
        assert!(!is_sdxl);
        assert_eq!(training_script(is_sdxl), "train_network.py");

        // SDXL이 생기면 SDXL 우선
        std::fs::write(ckpt.join("sd_xl_base_1.0.safetensors"), b"m").unwrap();
        let (path, is_sdxl) = resolve_base_model(root.path()).unwrap();
        assert!(path.ends_with("sd_xl_base_1.0.safetensors"));
        assert!(is_sdxl);
        assert_eq!(training_script(is_sdxl), "sdxl_train_network.py");
    }

    #[test]
    fn collect_artifact_moves_to_loras_without_overwriting_existing() {
        let root = tempfile::tempdir().unwrap();
        let dataset = root.path().join("datasets/ds1");
        make_dataset(&dataset, 1);
        let layout = prepare_kohya_layout(root.path(), "job1", &dataset, &fast(), "brand").unwrap();

        // 같은 이름의 기존 LoRA가 있으면 덮어쓰지 않고 -2로 비켜 간다
        std::fs::create_dir_all(root.path().join("models/loras")).unwrap();
        std::fs::write(
            root.path().join("models/loras/brand-job1.safetensors"),
            b"old",
        )
        .unwrap();
        std::fs::write(layout.output_dir.join("brand-job1.safetensors"), b"new").unwrap();

        let rel = collect_lora_artifact(root.path(), &layout, "brand-job1").unwrap();
        assert_eq!(rel, "models/loras/brand-job1-2.safetensors");
        assert_eq!(
            std::fs::read(root.path().join("models/loras/brand-job1.safetensors")).unwrap(),
            b"old",
            "기존 파일은 보존"
        );
        assert!(root.path().join(&rel).exists());

        // 산출물이 없으면 명확한 에러
        assert_eq!(
            collect_lora_artifact(root.path(), &layout, "brand-job1")
                .unwrap_err()
                .code,
            "E_TRAIN_NO_ARTIFACT"
        );
    }

    /// 가짜 kohya(쉘 스크립트)로 진행 파싱→업데이트→종료 드레인→산출물 수집
    /// 전체 흐름 검증. 마지막 진행줄·샘플은 개행 직후 즉시 종료해 종료 드레인
    /// 경로도 함께 확인한다.
    #[tokio::test]
    async fn fake_kohya_end_to_end_progress_samples_and_artifact() {
        let root = tempfile::tempdir().unwrap();
        let dataset = root.path().join("datasets/ds1");
        make_dataset(&dataset, 2);

        // 러너가 요구하는 파일 배치: venv python(가짜), kohya 스크립트, 기반 모델
        let venv_bin = root.path().join("runtime/venv/bin");
        std::fs::create_dir_all(&venv_bin).unwrap();
        let kohya_dir = crate::bootstrap::kohya::kohya_dir(root.path());
        std::fs::create_dir_all(&kohya_dir).unwrap();
        std::fs::create_dir_all(root.path().join("models/checkpoints")).unwrap();
        std::fs::write(
            root.path()
                .join("models/checkpoints/sd_xl_base_1.0.safetensors"),
            b"model",
        )
        .unwrap();

        let layout = prepare_kohya_layout(root.path(), "job1", &dataset, &fast(), "brand").unwrap();
        // 가짜 파이썬: 진행줄 출력 → 샘플 PNG 생성 → 산출물 생성 → 즉시 종료
        let fake = venv_bin.join("python");
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\n\
                 echo 'epoch 1/2'\n\
                 echo 'steps:  50%|#####     | 5/10 [00:01<00:01, 5.0it/s, avg_loss=0.2]' 1>&2\n\
                 touch '{sample}/job1_e000001.png'\n\
                 sleep 0.1\n\
                 echo 'epoch 2/2'\n\
                 printf lora > '{out}/brand-job1.safetensors'\n\
                 echo 'steps: 100%|##########| 10/10 [00:02<00:00, 5.0it/s, avg_loss=0.1]' 1>&2\n",
                sample = layout.sample_dir.display(),
                out = layout.output_dir.display(),
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(kohya_dir.join("sdxl_train_network.py"), b"# fake").unwrap();

        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut updates = Vec::new();
        let rel = run_training(root.path(), &layout, &fast(), "brand-job1", &rx, |u| {
            updates.push(u);
        })
        .await
        .unwrap();

        assert_eq!(rel, "models/loras/brand-job1.safetensors");
        assert!(root.path().join(&rel).exists());
        // 진행 업데이트: 50%와, 종료 직전 출력된 마지막 100%도 (종료 드레인) 도착
        assert!(updates.iter().any(|u| matches!(
            u,
            TrainUpdate::Progress { progress, .. } if (*progress - 0.5).abs() < 1e-9
        )));
        assert!(updates.iter().any(|u| matches!(
            u,
            TrainUpdate::Progress { progress, .. } if (*progress - 1.0).abs() < 1e-9
        )));
        // 샘플 이벤트 (종료 후 최종 스캔 포함 경로)
        assert!(updates
            .iter()
            .any(|u| matches!(u, TrainUpdate::Sample { .. })));
        // train.log에 원문 기록
        assert!(std::fs::read_to_string(layout.work_dir.join("train.log"))
            .unwrap()
            .contains("avg_loss=0.2"));
    }

    /// 취소: sleep하는 가짜 프로세스를 띄우고 취소 채널로 종료.
    #[tokio::test]
    async fn cancel_kills_process_and_returns_e_canceled() {
        let root = tempfile::tempdir().unwrap();
        let dataset = root.path().join("datasets/ds1");
        make_dataset(&dataset, 1);
        let venv_bin = root.path().join("runtime/venv/bin");
        std::fs::create_dir_all(&venv_bin).unwrap();
        let kohya_dir = crate::bootstrap::kohya::kohya_dir(root.path());
        std::fs::create_dir_all(&kohya_dir).unwrap();
        std::fs::create_dir_all(root.path().join("models/checkpoints")).unwrap();
        std::fs::write(
            root.path()
                .join("models/checkpoints/sd_xl_base_1.0.safetensors"),
            b"model",
        )
        .unwrap();
        let fake = venv_bin.join("python");
        std::fs::write(&fake, "#!/bin/sh\nsleep 30\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(kohya_dir.join("sdxl_train_network.py"), b"# fake").unwrap();

        let layout = prepare_kohya_layout(root.path(), "job1", &dataset, &fast(), "t").unwrap();
        let (tx, rx) = tokio::sync::watch::channel(false);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = tx.send(true);
        });
        let err = run_training(root.path(), &layout, &fast(), "t-job1", &rx, |_| {})
            .await
            .unwrap_err();
        assert_eq!(err.code, "E_CANCELED");
    }
}
