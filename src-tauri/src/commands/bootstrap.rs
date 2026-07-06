//! bootstrap_status / bootstrap_run (TAD §5).
//!
//! long-running 규칙(CLAUDE.md 4): `bootstrap_run`은 jobId만 반환하고
//! 진행은 `bootstrap://progress` 이벤트로 push한다.

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::bootstrap::state::{BootstrapState, ModelProfile, Step};
use crate::bootstrap::{models, Bootstrapper};
use crate::error::AppError;
use crate::paths;

/// 중복 실행 방지용 관리 상태.
#[derive(Default)]
pub struct BootstrapJob(pub Mutex<Option<String>>);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapStatus {
    pub step: Step,
    pub progress: f64,
    pub ready: bool,
    pub suggested_profile: ModelProfile,
}

fn data_root(app: &AppHandle) -> Result<std::path::PathBuf, AppError> {
    let base = app
        .path()
        .data_dir()
        .map_err(|e| AppError::with_detail("E_PATH", "앱 데이터 폴더를 찾지 못했어요.", e))?;
    Ok(paths::app_data_root(&base))
}

/// macOS RAM 크기 감지 (sysctl). 실패 시 보수적으로 light 제안.
fn detect_ram_bytes() -> u64 {
    std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

#[tauri::command]
pub async fn bootstrap_status(app: AppHandle) -> Result<BootstrapStatus, AppError> {
    let root = data_root(&app)?;
    let st = BootstrapState::load(&Bootstrapper::new(root).state_path());
    Ok(BootstrapStatus {
        step: st.step,
        progress: st.step.base_progress(),
        ready: st.is_ready(),
        suggested_profile: models::suggest_profile(detect_ram_bytes()),
    })
}

/// 설치 실패 시 `logs/bootstrap.log`를 기본 앱(Finder 연동)으로 연다 (TAD §7, 04 §4.6).
#[tauri::command]
pub async fn bootstrap_open_log(app: AppHandle) -> Result<(), AppError> {
    let log_path = data_root(&app)?.join("logs").join("bootstrap.log");
    if !log_path.exists() {
        return Err(AppError::new(
            "E_NO_LOG",
            "아직 설치 로그가 없어요. 설치를 시작하면 만들어져요.",
        ));
    }
    std::process::Command::new("/usr/bin/open")
        .arg(&log_path)
        .spawn()
        .map_err(|e| AppError::with_detail("E_OPEN_LOG", "로그 파일을 열지 못했어요.", e))?;
    Ok(())
}

#[tauri::command]
pub async fn bootstrap_run(
    app: AppHandle,
    job: State<'_, BootstrapJob>,
    model_profile: ModelProfile,
) -> Result<String, AppError> {
    // 중복 실행 방지
    {
        let mut running = job
            .0
            .lock()
            .map_err(|_| AppError::new("E_STATE", "내부 상태 잠금에 실패했어요."))?;
        if running.is_some() {
            return Err(AppError::new(
                "E_BOOTSTRAP_RUNNING",
                "이미 설치가 진행 중이에요.",
            ));
        }
        *running = Some(uuid::Uuid::new_v4().to_string());
    }

    let job_id = job
        .0
        .lock()
        .map_err(|_| AppError::new("E_STATE", "내부 상태 잠금에 실패했어요."))?
        .clone()
        .unwrap_or_default();

    let root = data_root(&app)?;
    let app2 = app.clone();
    let emit = Arc::new(move |ev: crate::bootstrap::ProgressEvent| {
        let _ = app2.emit("bootstrap://progress", &ev);
    });

    let app3 = app.clone();
    tauri::async_runtime::spawn(async move {
        let bootstrapper = Bootstrapper::new(root);
        // 에러는 run() 내부에서 error 필드가 담긴 progress 이벤트로 이미 알림
        let result = bootstrapper.run(model_profile, emit).await;
        match result {
            Ok(()) => {
                // 설치 완료 → 엔진 자동 기동 (TAD §6)
                if let Some(engine) = app3.try_state::<crate::commands::engine::Engine>() {
                    if engine.config.is_installed() {
                        if let Err(e) = engine.manager.start().await {
                            eprintln!("설치 후 엔진 기동 실패: {e}");
                        }
                    }
                }
            }
            Err(e) => eprintln!("bootstrap 실패: {e}"),
        }
        // 완료/실패 후 재실행 허용
        if let Some(job) = app3.try_state::<BootstrapJob>() {
            if let Ok(mut running) = job.0.lock() {
                *running = None;
            }
        }
    });

    Ok(job_id)
}
