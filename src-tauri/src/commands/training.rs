//! kohya_install_status / kohya_install_run (TAD §5, T6.1).
//! dataset_create / caption_dataset / dataset_save_captions (TAD §5, T6.2).
//! training_start / training_cancel (TAD §5, T6.3).
//!
//! long-running 규칙(CLAUDE.md 4): `kohya_install_run`/`training_start`는
//! jobId만 반환하고 진행은 `train://` 이벤트로 push한다. 취소는 generate와
//! 같은 watch 채널 레지스트리 패턴. `caption_dataset`은 essence_create와
//! 같은 계약 형태 — 결과를 직접 반환하고 진행 로그만 `caption://progress`
//! 이벤트로 중계한다(jobId 패턴이 아님).

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::bootstrap::kohya;
use crate::db::Db;
use crate::error::AppError;
use crate::paths;
use crate::training;
use crate::training::profiles::{load_profile, ProfileKind};
use crate::training::runner::{prepare_kohya_layout, run_training, sanitize_trigger, TrainUpdate};

#[derive(Default)]
pub struct KohyaInstallJob(pub Mutex<Option<String>>);

/// 실행 중인 학습 잡의 취소 핸들 레지스트리 (generate의 GenJobs와 동일 패턴).
#[derive(Default)]
pub struct TrainJobs(pub Mutex<HashMap<String, tokio::sync::watch::Sender<bool>>>);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KohyaInstallStatus {
    pub installed: bool,
}

/// `train://install_progress` 이벤트 페이로드.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KohyaInstallProgressEvent {
    job_id: String,
    done: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<AppError>,
}

fn data_root(app: &AppHandle) -> Result<std::path::PathBuf, AppError> {
    Ok(paths::app_data_root(&app.path().data_dir().map_err(
        |e| AppError::with_detail("E_PATH", "앱 데이터 폴더를 찾지 못했어요.", e),
    )?))
}

#[tauri::command]
pub async fn kohya_install_status(app: AppHandle) -> Result<KohyaInstallStatus, AppError> {
    Ok(KohyaInstallStatus {
        installed: kohya::is_kohya_installed(&data_root(&app)?),
    })
}

#[tauri::command]
pub async fn kohya_install_run(
    app: AppHandle,
    job: State<'_, KohyaInstallJob>,
) -> Result<String, AppError> {
    let job_id = {
        let mut running = job
            .0
            .lock()
            .map_err(|_| AppError::new("E_STATE", "내부 상태 잠금에 실패했어요."))?;
        if running.is_some() {
            return Err(AppError::new(
                "E_KOHYA_INSTALL_RUNNING",
                "이미 학습 도구를 설치하는 중이에요.",
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        *running = Some(id.clone());
        id
    };

    let root = data_root(&app)?;
    let app2 = app.clone();
    let jid = job_id.clone();
    tauri::async_runtime::spawn(async move {
        let result = kohya::ensure_kohya_installed(&root).await;
        let event = match &result {
            Ok(()) => KohyaInstallProgressEvent {
                job_id: jid.clone(),
                done: true,
                message: "학습 도구 설치가 끝났어요.".to_string(),
                error: None,
            },
            Err(e) => KohyaInstallProgressEvent {
                job_id: jid.clone(),
                done: true,
                message: e.message.clone(),
                error: Some(e.clone()),
            },
        };
        let _ = app2.emit("train://install_progress", &event);
        if let Some(job) = app2.try_state::<KohyaInstallJob>() {
            if let Ok(mut running) = job.0.lock() {
                *running = None;
            }
        }
    });

    Ok(job_id)
}

/// 원본은 레포 루트 `python/caption.py` (TAD §2).
pub const CAPTION_PY: &str = include_str!("../../../python/caption.py");

/// WD14 태거 다운로드(첫 실행)를 포함해 넉넉히 — essence_create와 동일 값.
const CAPTION_TIMEOUT_SECS: u64 = 1800;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetInfo {
    pub id: String,
    pub dir: String,
    pub files: Vec<String>,
}

/// 드롭한 이미지(절대 경로)를 datasets/{id}/로 복사해 새 데이터셋을 만든다.
#[tauri::command]
pub async fn dataset_create(
    app: AppHandle,
    image_paths: Vec<String>,
) -> Result<DatasetInfo, AppError> {
    let data_root = paths::app_data_root(
        &app.path()
            .data_dir()
            .map_err(|e| AppError::with_detail("E_PATH", "앱 데이터 폴더를 찾지 못했어요.", e))?,
    );
    let id = uuid::Uuid::new_v4().to_string();
    let files = training::copy_dataset_images(&data_root, &id, &image_paths)?;
    let dir = training::dataset_dir(&data_root, &id);
    Ok(DatasetInfo {
        id,
        dir: dir.to_string_lossy().into_owned(),
        files,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionItem {
    pub file: String,
    pub caption: String,
}

/// caption.py stdout(JSON 한 줄) 파싱.
pub fn parse_caption_output(line: &str) -> Result<Vec<CaptionItem>, String> {
    #[derive(Deserialize)]
    struct PyOut {
        ok: bool,
        items: Option<Vec<CaptionItem>>,
        error: Option<String>,
        detail: Option<String>,
    }
    let parsed: PyOut =
        serde_json::from_str(line.trim()).map_err(|e| format!("잘못된 출력: {e}"))?;
    if parsed.ok {
        Ok(parsed.items.unwrap_or_default())
    } else {
        Err(format!(
            "{}: {}",
            parsed.error.unwrap_or_else(|| "unknown".into()),
            parsed.detail.unwrap_or_default()
        ))
    }
}

/// dir 안의 이미지에 WD14 태그 캡션을 자동 생성한다(결과를 직접 반환, 진행은
/// `caption://progress` 이벤트 — essence_create와 동일한 계약 형태).
#[tauri::command]
pub async fn caption_dataset(app: AppHandle, dir: String) -> Result<Vec<CaptionItem>, AppError> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let data_root = paths::app_data_root(
        &app.path()
            .data_dir()
            .map_err(|e| AppError::with_detail("E_PATH", "앱 데이터 폴더를 찾지 못했어요.", e))?,
    );
    let python = data_root.join("runtime/venv/bin/python");
    if !python.exists() {
        return Err(AppError::new(
            "E_CAPTION_NOT_READY",
            "캡션 도구가 아직 설치되지 않았어요. 처음 사용 설정(엔진 설치)을 마치면 쓸 수 있어요.",
        ));
    }
    let script_dir = data_root.join("runtime");
    std::fs::create_dir_all(&script_dir)?;
    let script = script_dir.join("caption.py");
    paths::write_if_changed(&script, CAPTION_PY)?;

    let mut child = tokio::process::Command::new(&python)
        .arg(&script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            AppError::with_detail("E_CAPTION_SPAWN", "캡션 생성을 시작하지 못했어요.", e)
        })?;

    let input = serde_json::json!({
        "dir": dir,
        "hfHome": data_root.join("models/hf").to_string_lossy(),
    })
    .to_string()
        + "\n";
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(input.as_bytes()).await.map_err(|e| {
            AppError::with_detail("E_CAPTION_SPAWN", "캡션 입력을 전달하지 못했어요.", e)
        })?;
    }
    drop(child.stdin.take());

    if let Some(stderr) = child.stderr.take() {
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = app2.emit("caption://progress", serde_json::json!({ "message": line }));
            }
        });
    }

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(CAPTION_TIMEOUT_SECS),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| {
        AppError::new(
            "E_CAPTION_TIMEOUT",
            "캡션 생성이 너무 오래 걸려 중단했어요. 이미지 수를 줄여 다시 시도해 주세요.",
        )
    })?
    .map_err(|e| {
        AppError::with_detail(
            "E_CAPTION_SPAWN",
            "캡션 생성 중 문제가 생겼어요. 다시 시도해 주세요.",
            e,
        )
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_caption_output(stdout.lines().next().unwrap_or("")).map_err(|detail| {
        AppError::with_detail("E_CAPTION_FAILED", "캡션 생성에 실패했어요.", detail)
    })
}

/// 캡션을 kohya sd-scripts 관례({basename}.txt)로 데이터셋 폴더에 저장한다.
#[tauri::command]
pub async fn dataset_save_captions(dir: String, items: Vec<CaptionItem>) -> Result<(), AppError> {
    let pairs: Vec<(String, String)> = items
        .into_iter()
        .map(|item| (item.file, item.caption))
        .collect();
    training::save_captions(std::path::Path::new(&dir), &pairs)
}

/// training_start 입력 (TAD §5 — camelCase). triggerWord는 캡션·폴더 규약과
/// 완료 시 스타일 등록(T6.4)에 쓰인다.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainingStartArgs {
    pub style_id: String,
    pub dataset_dir: String,
    pub profile: ProfileKind,
    pub trigger_word: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrainProgressEvent {
    job_id: String,
    /// 0.0 ~ 1.0
    progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    eta_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    loss: Option<f64>,
    /// [현재, 전체] — epoch 경계 이후부터 채워짐
    #[serde(skip_serializing_if = "Option::is_none")]
    epoch: Option<[u32; 2]>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrainSampleEvent {
    job_id: String,
    /// 절대 경로 (작업 폴더 안 — 학습 끝나면 정리될 수 있음)
    image_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrainDoneEvent {
    job_id: String,
    style_id: String,
    /// 데이터 루트 기준 상대 경로 (models/loras/…)
    lora_path: String,
    trigger_word: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrainErrorEvent {
    job_id: String,
    error: AppError,
}

#[tauri::command]
pub async fn training_start(
    app: AppHandle,
    jobs: State<'_, TrainJobs>,
    db: State<'_, Db>,
    args: TrainingStartArgs,
) -> Result<String, AppError> {
    let root = data_root(&app)?;
    if !kohya::is_kohya_installed(&root) {
        return Err(AppError::new(
            "E_KOHYA_NOT_INSTALLED",
            "학습 도구가 아직 설치되지 않았어요. 학습 시작 전에 설치를 마쳐 주세요.",
        ));
    }

    let job_id = uuid::Uuid::new_v4().to_string();
    let profile = load_profile(args.profile)?;
    // 저렴한 사전 검증만 command에서 — 데이터셋 복사(수백 MB 가능)는 spawn된
    // 태스크에서 한다 (CLAUDE.md 규칙 4: 블로킹 command 금지)
    let dataset_dir = std::path::PathBuf::from(&args.dataset_dir);
    training::runner::count_dataset_images(&dataset_dir)?;

    // DB insert를 레지스트리 등록보다 먼저 — insert 실패 시 취소 핸들이
    // 지도에 고아로 남는 누수 방지
    let now = chrono::Utc::now().timestamp_millis();
    let job_row = crate::db::models::TrainingJob {
        id: job_id.clone(),
        style_id: args.style_id.clone(),
        status: "training".to_string(),
        progress: 0.0,
        eta_seconds: None,
        params_json: serde_json::to_string(&profile).ok(),
        error: None,
        started_at: Some(now),
        finished_at: None,
    };
    db.insert_training_job(&job_row)
        .await
        .map_err(|e| AppError::with_detail("E_DB", "학습 기록을 저장하지 못했어요.", e))?;

    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    {
        let mut map = jobs
            .0
            .lock()
            .map_err(|_| AppError::new("E_STATE", "내부 상태 잠금에 실패했어요."))?;
        map.insert(job_id.clone(), cancel_tx);
    }

    let output_name = format!(
        "{}-{}",
        sanitize_trigger(&args.trigger_word),
        &job_id[..8.min(job_id.len())]
    );
    let app2 = app.clone();
    let job_id2 = job_id.clone();
    let style_id = args.style_id.clone();
    let trigger_word = args.trigger_word.clone();
    tauri::async_runtime::spawn(async move {
        let db = app2.state::<Db>().inner().clone();

        // 업데이트 펌프: 이벤트 방출 + DB 기록을 한 소비자에서 순차 처리 —
        // 업데이트마다 태스크를 spawn하면 순서 보장이 없어 늦은 progress
        // 기록이 종료 상태(done/failed)를 'training'으로 되돌릴 수 있다
        let (update_tx, mut update_rx) = tokio::sync::mpsc::unbounded_channel::<TrainUpdate>();
        let pump = {
            let app3 = app2.clone();
            let db2 = db.clone();
            let id = job_id2.clone();
            tauri::async_runtime::spawn(async move {
                let mut last_db_percent = -1i64;
                // tqdm은 스텝마다 라인을 내므로 이벤트도 1%/100ms로 코얼레싱 (T9.2 §P1.6)
                let mut coalescer = crate::engine::generation::ProgressCoalescer::new();
                while let Some(update) = update_rx.recv().await {
                    match update {
                        TrainUpdate::Progress {
                            progress,
                            eta_seconds,
                            loss,
                            epoch,
                        } => {
                            if coalescer.should_emit(progress, false) {
                                let _ = app3.emit(
                                    "train://progress",
                                    &TrainProgressEvent {
                                        job_id: id.clone(),
                                        progress,
                                        eta_seconds,
                                        loss,
                                        epoch: epoch.map(|(c, t)| [c, t]),
                                    },
                                );
                            }
                            // DB 기록은 1% 단위 스로틀
                            let percent = (progress * 100.0) as i64;
                            if percent != last_db_percent {
                                last_db_percent = percent;
                                let _ = db2
                                    .update_training_progress(
                                        &id,
                                        "training",
                                        progress,
                                        eta_seconds,
                                    )
                                    .await;
                            }
                        }
                        TrainUpdate::Sample { image_path } => {
                            let _ = app3.emit(
                                "train://sample",
                                &TrainSampleEvent {
                                    job_id: id.clone(),
                                    image_path,
                                },
                            );
                        }
                    }
                }
            })
        };

        // 데이터셋 복사(블로킹 IO)는 여기서 — jobId는 이미 반환됨
        let result = match tauri::async_runtime::spawn_blocking({
            let root = root.clone();
            let job_id = job_id2.clone();
            let profile = profile.clone();
            let trigger = trigger_word.clone();
            move || prepare_kohya_layout(&root, &job_id, &dataset_dir, &profile, &trigger)
        })
        .await
        .map_err(|e| AppError::with_detail("E_TRAIN_SPAWN", "학습 준비에 실패했어요.", e))
        .and_then(|r| r)
        {
            Ok(layout) => {
                run_training(&root, &layout, &profile, &output_name, &cancel_rx, |u| {
                    let _ = update_tx.send(u);
                })
                .await
            }
            Err(e) => Err(e),
        };
        // 펌프가 큐에 남은 업데이트를 모두 DB에 기록한 뒤에 종료 상태를 쓴다
        drop(update_tx);
        let _ = pump.await;

        let finished_at = chrono::Utc::now().timestamp_millis();
        match result {
            Ok(lora_path) => {
                let _ = db
                    .finish_training_job(&job_id2, "done", None, finished_at)
                    .await;
                let _ = app2.emit(
                    "train://done",
                    &TrainDoneEvent {
                        job_id: job_id2.clone(),
                        style_id,
                        lora_path,
                        trigger_word,
                    },
                );
            }
            Err(error) => {
                let status = if error.code == "E_CANCELED" {
                    "canceled"
                } else {
                    "failed"
                };
                let _ = db
                    .finish_training_job(&job_id2, status, Some(&error.message), finished_at)
                    .await;
                let _ = app2.emit(
                    "train://error",
                    &TrainErrorEvent {
                        job_id: job_id2.clone(),
                        error,
                    },
                );
            }
        }
        if let Some(jobs) = app2.try_state::<TrainJobs>() {
            if let Ok(mut map) = jobs.0.lock() {
                map.remove(&job_id2);
            }
        }
    });

    Ok(job_id)
}

#[tauri::command]
pub async fn training_cancel(jobs: State<'_, TrainJobs>, job_id: String) -> Result<(), AppError> {
    let map = jobs
        .0
        .lock()
        .map_err(|_| AppError::new("E_STATE", "내부 상태 잠금에 실패했어요."))?;
    match map.get(&job_id) {
        Some(tx) => {
            let _ = tx.send(true);
            Ok(())
        }
        None => Err(AppError::new(
            "E_JOB_NOT_FOUND",
            "진행 중인 학습을 찾을 수 없어요.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_output_parsing() {
        let ok = parse_caption_output(
            r#"{"ok":true,"items":[{"file":"a.png","caption":"cat, flat color"}]}"#,
        )
        .unwrap();
        assert_eq!(
            ok,
            vec![CaptionItem {
                file: "a.png".to_string(),
                caption: "cat, flat color".to_string()
            }]
        );

        let err = parse_caption_output(r#"{"ok":false,"error":"bad_input","detail":"no dir"}"#)
            .unwrap_err();
        assert!(err.contains("bad_input"));
        assert!(parse_caption_output("garbage").is_err());
    }
}
