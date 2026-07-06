//! kohya_install_status / kohya_install_run (TAD §5, T6.1).
//!
//! long-running 규칙(CLAUDE.md 4): `kohya_install_run`은 jobId만 반환하고
//! 진행은 `train://install_progress` 이벤트로 push한다. bootstrap_run
//! (commands/bootstrap.rs)과 같은 중복 실행 방지 패턴.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::bootstrap::kohya;
use crate::error::AppError;
use crate::paths;

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
