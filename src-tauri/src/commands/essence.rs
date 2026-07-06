//! essence_create (TAD §5) — 참조 이미지 → 에센스 프롬프트 (T4.2).
//!
//! python/essence.py(내장)를 venv 파이썬으로 실행한다. 분석 로그(stderr)를
//! `essence://progress` 이벤트로 흘려보내고, 완료 결과를 그대로 반환한다
//! (TAD §5 계약 — jobId 패턴이 아님).

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::AppError;
use crate::paths;

/// 원본은 레포 루트 `python/essence.py` (TAD §2).
pub const ESSENCE_PY: &str = include_str!("../../../python/essence.py");

/// 첫 실행은 Florence-2 다운로드(~0.9GB)가 포함될 수 있어 매우 넉넉히.
const ESSENCE_TIMEOUT_SECS: u64 = 1800;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EssenceArgs {
    pub image_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EssenceResult {
    pub essence_prompt: String,
    pub tags: Vec<String>,
    #[serde(default)]
    pub captions: Vec<String>,
}

/// essence.py stdout(JSON 한 줄) 파싱.
pub fn parse_essence_output(line: &str) -> Result<EssenceResult, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct PyOut {
        ok: bool,
        essence_prompt: Option<String>,
        tags: Option<Vec<String>>,
        captions: Option<Vec<String>>,
        error: Option<String>,
        detail: Option<String>,
    }
    let parsed: PyOut =
        serde_json::from_str(line.trim()).map_err(|e| format!("잘못된 출력: {e}"))?;
    if parsed.ok {
        Ok(EssenceResult {
            essence_prompt: parsed.essence_prompt.unwrap_or_default(),
            tags: parsed.tags.unwrap_or_default(),
            captions: parsed.captions.unwrap_or_default(),
        })
    } else {
        Err(format!(
            "{}: {}",
            parsed.error.unwrap_or_else(|| "unknown".into()),
            parsed.detail.unwrap_or_default()
        ))
    }
}

#[tauri::command]
pub async fn essence_create(app: AppHandle, args: EssenceArgs) -> Result<EssenceResult, AppError> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let data_root = paths::app_data_root(
        &app.path()
            .data_dir()
            .map_err(|e| AppError::with_detail("E_PATH", "앱 데이터 폴더를 찾지 못했어요.", e))?,
    );
    let python = data_root.join("runtime/venv/bin/python");
    if !python.exists() {
        return Err(AppError::new(
            "E_ESSENCE_NOT_READY",
            "분석 도구가 아직 설치되지 않았어요. 처음 사용 설정(엔진 설치)을 마치면 쓸 수 있어요.",
        ));
    }
    let script_dir = data_root.join("runtime");
    std::fs::create_dir_all(&script_dir)?;
    let script = script_dir.join("essence.py");
    std::fs::write(&script, ESSENCE_PY)?;

    let mut child = tokio::process::Command::new(&python)
        .arg(&script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::with_detail("E_ESSENCE_SPAWN", "분석을 시작하지 못했어요.", e))?;

    let input = serde_json::json!({
        "images": args.image_paths,
        "hfHome": data_root.join("models/hf").to_string_lossy(),
    })
    .to_string()
        + "\n";
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(input.as_bytes()).await.map_err(|e| {
            AppError::with_detail("E_ESSENCE_SPAWN", "분석 입력을 전달하지 못했어요.", e)
        })?;
    }
    drop(child.stdin.take());

    // stderr 로그를 진행 이벤트로 중계 (essence://progress)
    if let Some(stderr) = child.stderr.take() {
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = app2.emit("essence://progress", serde_json::json!({ "message": line }));
            }
        });
    }

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(ESSENCE_TIMEOUT_SECS),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| {
        AppError::new(
            "E_ESSENCE_TIMEOUT",
            "분석이 너무 오래 걸려 중단했어요. 이미지 수를 줄여 다시 시도해 주세요.",
        )
    })?
    .map_err(|e| {
        AppError::with_detail(
            "E_ESSENCE_SPAWN",
            "이미지 분석 중 문제가 생겼어요. 다시 시도해 주세요.",
            e,
        )
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_essence_output(stdout.lines().next().unwrap_or("")).map_err(|detail| {
        AppError::with_detail("E_ESSENCE_FAILED", "스타일 분석에 실패했어요.", detail)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn essence_output_parsing() {
        let ok = parse_essence_output(
            r#"{"ok":true,"essencePrompt":"flat color, simple background","tags":["flat color"],"captions":["a cat"]}"#,
        )
        .unwrap();
        assert_eq!(ok.essence_prompt, "flat color, simple background");
        assert_eq!(ok.tags, vec!["flat color"]);
        assert_eq!(ok.captions, vec!["a cat"]);

        let err = parse_essence_output(r#"{"ok":false,"error":"bad_input","detail":"3~10"}"#)
            .unwrap_err();
        assert!(err.contains("bad_input"));
        assert!(parse_essence_output("garbage").is_err());
    }
}
