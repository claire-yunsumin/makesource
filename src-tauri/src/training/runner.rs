//! kohya 학습 잡 러너 (TAD §8, T6.3).
//!
//! 흐름: 평평한 데이터셋(`datasets/{id}/`, D-009)을 kohya 규약
//! `training/{jobId}/img/{repeats}_{trigger}/`로 복사 → venv 파이썬으로
//! `sdxl_train_network.py` 실행 → stderr/stdout을 parser로 해석해 진행/샘플
//! 업데이트 콜백 → 완료 시 .safetensors를 `models/loras/`로 이동(TAD §8 5;
//! styles.json 등록은 T6.4). 취소는 watch 채널 → 프로세스 kill.

use std::path::{Path, PathBuf};

use crate::bootstrap::kohya;
use crate::error::AppError;
use crate::training::parser::{parse_line, split_carriage_lines, TrainEvent};
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

/// D-009: 평평한 데이터셋을 kohya 규약 폴더로 복사 (이미지 + {basename}.txt 캡션).
pub fn prepare_kohya_layout(
    data_root: &Path,
    job_id: &str,
    dataset_dir: &Path,
    repeats: u32,
    trigger: &str,
) -> Result<KohyaLayout, AppError> {
    if !dataset_dir.is_dir() {
        return Err(AppError::with_detail(
            "E_DATASET_NOT_FOUND",
            "학습 데이터셋 폴더를 찾을 수 없어요.",
            dataset_dir.display(),
        ));
    }
    let work_dir = data_root.join("training").join(job_id);
    let img_dir = work_dir
        .join("img")
        .join(format!("{repeats}_{}", sanitize_trigger(trigger)));
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
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_image = matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp");
        let is_caption = ext == "txt";
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
    if image_count == 0 {
        return Err(AppError::new(
            "E_DATASET_EMPTY",
            "데이터셋에 이미지가 없어요. 이미지를 추가한 뒤 다시 시도해 주세요.",
        ));
    }
    Ok(KohyaLayout {
        work_dir,
        img_dir,
        output_dir,
        sample_dir,
        image_count,
    })
}

/// sdxl_train_network.py 인자 조립 (순수 — 테스트 대상).
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

/// 최종 산출물(.safetensors)을 models/loras/로 이동하고 루트 기준 상대 경로 반환.
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
    let rel = format!("models/loras/{output_name}.safetensors");
    let dest = data_root.join(&rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(&src, &dest).or_else(|_| {
        // 볼륨 경계 등으로 rename 실패 시 복사 폴백
        std::fs::copy(&src, &dest).map(|_| ())
    })?;
    Ok(rel)
}

/// 학습 실행. 진행/샘플은 on_update로, 완료 시 LoRA 상대 경로 반환.
/// cancel_rx가 true가 되면 프로세스를 종료하고 E_CANCELED를 돌려준다.
#[allow(clippy::too_many_arguments)]
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
    let script = kohya::kohya_dir(data_root).join("sdxl_train_network.py");
    if !python.exists() || !script.exists() {
        return Err(AppError::new(
            "E_KOHYA_NOT_INSTALLED",
            "학습 도구가 아직 설치되지 않았어요. 학습 시작 전에 설치를 마쳐 주세요.",
        ));
    }
    let base_model = data_root.join("models/checkpoints/sd_xl_base_1.0.safetensors");
    if !base_model.exists() {
        return Err(AppError::new(
            "E_BASE_MODEL_MISSING",
            "기반 모델(SDXL)이 없어요. 처음 사용 설정에서 표준 프로파일을 설치해 주세요.",
        ));
    }

    let args = build_kohya_args(layout, profile, &base_model, output_name);
    let log_path = layout.work_dir.join("train.log");
    let mut log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok();

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
    let mut out_buf = [0u8; 4096];
    let mut err_buf = [0u8; 4096];
    let mut cancel = cancel_rx.clone();
    let mut last_epoch: Option<(u32, u32)> = None;
    let mut seen_samples: std::collections::HashSet<PathBuf> = Default::default();

    let handle_chunk = |chunk: &str,
                        log_file: &mut Option<std::fs::File>,
                        on_update: &mut dyn FnMut(TrainUpdate),
                        last_epoch: &mut Option<(u32, u32)>| {
        if let Some(f) = log_file {
            use std::io::Write;
            let _ = f.write_all(chunk.as_bytes());
        }
        for line in split_carriage_lines(chunk) {
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
                Some(TrainEvent::Saved { .. }) | None => {}
            }
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
                    Ok(0) => { stdout = None; }
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&out_buf[..n]).into_owned();
                        handle_chunk(&chunk, &mut log_file, &mut on_update, &mut last_epoch);
                    }
                    Err(_) => { stdout = None; }
                }
            }
            read = async {
                match stderr.as_mut() {
                    Some(err) => err.read(&mut err_buf).await,
                    None => std::future::pending().await,
                }
            } => {
                match read {
                    Ok(0) => { stderr = None; }
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&err_buf[..n]).into_owned();
                        handle_chunk(&chunk, &mut log_file, &mut on_update, &mut last_epoch);
                    }
                    Err(_) => { stderr = None; }
                }
            }
            exit = child.wait() => {
                break exit.map_err(|e| {
                    AppError::with_detail("E_TRAIN_SPAWN", "학습 프로세스 오류예요.", e)
                })?;
            }
        }

        // 새 샘플 이미지 스캔 (epoch 샘플 스트립 — 04 §4.3 ④)
        if let Ok(entries) = std::fs::read_dir(&layout.sample_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_png = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("png"));
                if is_png && seen_samples.insert(path.clone()) {
                    on_update(TrainUpdate::Sample {
                        image_path: path.to_string_lossy().into_owned(),
                    });
                }
            }
        }
    };

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

    #[test]
    fn sanitize_trigger_keeps_alnum_lowercase_with_fallback() {
        assert_eq!(sanitize_trigger("MyStyle"), "mystyle");
        assert_eq!(sanitize_trigger("우리 브랜드!"), "style");
        assert_eq!(sanitize_trigger("brand-2 tone"), "brand2tone");
    }

    #[test]
    fn prepare_layout_copies_images_and_captions_into_kohya_convention() {
        let root = tempfile::tempdir().unwrap();
        let dataset = root.path().join("datasets/ds1");
        make_dataset(&dataset, 3);
        std::fs::write(dataset.join("notes.md"), b"skip me").unwrap();

        let layout = prepare_kohya_layout(root.path(), "job1", &dataset, 10, "MyStyle").unwrap();
        assert_eq!(layout.image_count, 3);
        assert!(layout.img_dir.ends_with("img/10_mystyle"));
        assert!(layout.img_dir.join("img-000.png").exists());
        assert!(layout.img_dir.join("img-000.txt").exists());
        assert!(!layout.img_dir.join("notes.md").exists());
        assert!(layout.sample_dir.is_dir());
    }

    #[test]
    fn prepare_layout_rejects_missing_or_empty_dataset() {
        let root = tempfile::tempdir().unwrap();
        let missing = root.path().join("nope");
        assert_eq!(
            prepare_kohya_layout(root.path(), "j", &missing, 10, "t")
                .unwrap_err()
                .code,
            "E_DATASET_NOT_FOUND"
        );
        let empty = root.path().join("datasets/empty");
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::write(empty.join("only.txt"), b"caption only").unwrap();
        assert_eq!(
            prepare_kohya_layout(root.path(), "j", &empty, 10, "t")
                .unwrap_err()
                .code,
            "E_DATASET_EMPTY"
        );
    }

    #[test]
    fn kohya_args_map_profile_and_layout() {
        let root = tempfile::tempdir().unwrap();
        let dataset = root.path().join("datasets/ds1");
        make_dataset(&dataset, 1);
        let layout = prepare_kohya_layout(root.path(), "job1", &dataset, 10, "brand").unwrap();
        let profile = load_profile(ProfileKind::Fast).unwrap();
        let args = build_kohya_args(
            &layout,
            &profile,
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
    }

    #[test]
    fn collect_artifact_moves_safetensors_to_loras_and_returns_relative_path() {
        let root = tempfile::tempdir().unwrap();
        let dataset = root.path().join("datasets/ds1");
        make_dataset(&dataset, 1);
        let layout = prepare_kohya_layout(root.path(), "job1", &dataset, 10, "brand").unwrap();
        std::fs::write(layout.output_dir.join("brand-job1.safetensors"), b"lora").unwrap();

        let rel = collect_lora_artifact(root.path(), &layout, "brand-job1").unwrap();
        assert_eq!(rel, "models/loras/brand-job1.safetensors");
        assert!(root.path().join(&rel).exists());
        assert!(!layout.output_dir.join("brand-job1.safetensors").exists());

        // 산출물이 없으면 명확한 에러
        assert_eq!(
            collect_lora_artifact(root.path(), &layout, "brand-job1")
                .unwrap_err()
                .code,
            "E_TRAIN_NO_ARTIFACT"
        );
    }

    /// 가짜 kohya(쉘 스크립트)로 진행 파싱→업데이트→산출물 수집 전체 흐름 검증.
    #[tokio::test]
    async fn fake_kohya_end_to_end_progress_and_artifact() {
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

        let layout = prepare_kohya_layout(root.path(), "job1", &dataset, 10, "brand").unwrap();
        // 가짜 파이썬: 진행줄 출력 → 샘플 PNG 생성 → 산출물 safetensors 생성
        let fake = venv_bin.join("python");
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\n\
                 echo 'epoch 1/2'\n\
                 echo 'steps:  50%|#####     | 5/10 [00:01<00:01, 5.0it/s, avg_loss=0.2]' 1>&2\n\
                 touch '{sample}/job1_e000001.png'\n\
                 sleep 0.2\n\
                 echo 'epoch 2/2'\n\
                 echo 'steps: 100%|##########| 10/10 [00:02<00:00, 5.0it/s, avg_loss=0.1]' 1>&2\n\
                 echo 'saving checkpoint: {out}/brand-job1.safetensors'\n\
                 printf lora > '{out}/brand-job1.safetensors'\n",
                sample = layout.sample_dir.display(),
                out = layout.output_dir.display(),
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(kohya_dir.join("sdxl_train_network.py"), b"# fake").unwrap();

        let profile = load_profile(ProfileKind::Fast).unwrap();
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let mut updates = Vec::new();
        let rel = run_training(root.path(), &layout, &profile, "brand-job1", &rx, |u| {
            updates.push(u);
        })
        .await
        .unwrap();

        assert_eq!(rel, "models/loras/brand-job1.safetensors");
        assert!(root.path().join(&rel).exists());
        // 진행 업데이트와 샘플 이벤트가 모두 도착
        assert!(updates.iter().any(|u| matches!(
            u,
            TrainUpdate::Progress { progress, .. } if (*progress - 0.5).abs() < 1e-9
        )));
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

        let layout = prepare_kohya_layout(root.path(), "job1", &dataset, 10, "t").unwrap();
        let profile = load_profile(ProfileKind::Fast).unwrap();
        let (tx, rx) = tokio::sync::watch::channel(false);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = tx.send(true);
        });
        let err = run_training(root.path(), &layout, &profile, "t-job1", &rx, |_| {})
            .await
            .unwrap_err();
        assert_eq!(err.code, "E_CANCELED");
    }
}
