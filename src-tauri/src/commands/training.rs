//! kohya_install_status / kohya_install_run (TAD §5, T6.1).
//! dataset_create / caption_dataset / dataset_save_captions (TAD §5, T6.2).
//!
//! long-running 규칙(CLAUDE.md 4): `kohya_install_run`은 jobId만 반환하고
//! 진행은 `train://install_progress` 이벤트로 push한다. bootstrap_run
//! (commands/bootstrap.rs)과 같은 중복 실행 방지 패턴. `caption_dataset`은
//! essence_create와 같은 계약 형태 — 결과를 직접 반환하고 진행 로그만
//! `caption://progress` 이벤트로 중계한다(jobId 패턴이 아님).

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::bootstrap::kohya;
use crate::error::AppError;
use crate::paths;
use crate::training;

#[derive(Default)]
pub struct KohyaInstallJob(pub Mutex<Option<String>>);

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
    std::fs::write(&script, CAPTION_PY)?;

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
    .map_err(|e| AppError::with_detail("E_CAPTION_SPAWN", "캡션 프로세스 오류예요.", e))?;

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
