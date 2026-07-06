//! generate / generate_cancel (TAD §5).
//!
//! generate는 jobId만 반환하고 진행·완료·실패는 `gen://progress|done|error`
//! 이벤트로 push한다 (CLAUDE.md 규칙 4).

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db::Db;
use crate::engine::generation::{run_generation, GenProgress, GenUpdate, GenerateRequest};
use crate::error::AppError;
use crate::paths;

/// 실행 중인 생성 잡의 취소 핸들 레지스트리.
#[derive(Default)]
pub struct GenJobs(pub Mutex<HashMap<String, tokio::sync::watch::Sender<bool>>>);

/// generate 입력 (TAD §5 계약 — camelCase).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateArgs {
    pub preset_id: String,
    pub style_id: Option<String>,
    pub keyword: String,
    pub count: Option<u32>,
    /// [width, height]
    pub size: Option<[u32; 2]>,
    pub seed: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenError {
    job_id: String,
    error: AppError,
}

#[tauri::command]
pub async fn generate(
    app: AppHandle,
    jobs: State<'_, GenJobs>,
    args: GenerateArgs,
) -> Result<String, AppError> {
    let job_id = uuid::Uuid::new_v4().to_string();
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    {
        let mut map = jobs
            .0
            .lock()
            .map_err(|_| AppError::new("E_STATE", "내부 상태 잠금에 실패했어요."))?;
        map.insert(job_id.clone(), cancel_tx);
    }

    let data_root = paths::app_data_root(
        &app.path()
            .data_dir()
            .map_err(|e| AppError::with_detail("E_PATH", "앱 데이터 폴더를 찾지 못했어요.", e))?,
    );
    let engine = app
        .try_state::<crate::commands::engine::Engine>()
        .ok_or_else(|| AppError::new("E_STATE", "엔진 상태를 찾을 수 없어요."))?;

    // 엔진 준비 가드 (04 §6: 원인 + 다음 행동을 정확히) — 미설치/미기동을
    // 생성 중 네트워크 오류("네트워크 연결에 문제가 있어요")로 뭉뚱그리지 않는다
    if !engine.config.is_installed() {
        return Err(AppError::new(
            "E_ENGINE_NOT_INSTALLED",
            "AI 엔진이 아직 설치되지 않았어요. 처음 사용 설정(엔진 설치)을 마치면 생성할 수 있어요.",
        ));
    }
    let health =
        crate::engine::check_health(&engine.manager, &engine.client, &engine.config.health_url())
            .await;
    if !health.running {
        return Err(AppError::new(
            "E_ENGINE_NOT_RUNNING",
            "엔진이 응답하지 않아요. 잠시 후 다시 시도하거나 앱을 재시작해 주세요.",
        ));
    }
    let base_url = engine.config.base_url();

    let req = GenerateRequest {
        preset_id: args.preset_id,
        style_id: args.style_id,
        keyword: args.keyword,
        count: args.count.unwrap_or(1),
        size: args.size.map(|[w, h]| (w, h)),
        seed: args.seed,
    };

    let app2 = app.clone();
    let job_id2 = job_id.clone();
    tauri::async_runtime::spawn(async move {
        let db = app2.state::<Db>().inner().clone();
        let mut last_progress = 0.0f64;
        let emit_update = |update: GenUpdate| {
            let (progress, notice) = match update {
                GenUpdate::Progress(p) => {
                    last_progress = p;
                    (p, None)
                }
                GenUpdate::Notice(text) => (last_progress, Some(text)),
            };
            let _ = app2.emit(
                "gen://progress",
                &GenProgress {
                    job_id: job_id2.clone(),
                    progress,
                    notice,
                },
            );
        };
        let result = run_generation(
            &job_id2,
            &data_root,
            &base_url,
            &db,
            &req,
            &cancel_rx,
            emit_update,
        )
        .await;

        match result {
            Ok(done) => {
                let _ = app2.emit("gen://done", &done);
            }
            Err(error) => {
                let _ = app2.emit(
                    "gen://error",
                    &GenError {
                        job_id: job_id2.clone(),
                        error,
                    },
                );
            }
        }
        // 잡 종료 → 레지스트리 정리
        if let Some(jobs) = app2.try_state::<GenJobs>() {
            if let Ok(mut map) = jobs.0.lock() {
                map.remove(&job_id2);
            }
        }
    });

    Ok(job_id)
}

#[tauri::command]
pub async fn generate_cancel(jobs: State<'_, GenJobs>, job_id: String) -> Result<(), AppError> {
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
            "진행 중인 작업을 찾을 수 없어요.",
        )),
    }
}
