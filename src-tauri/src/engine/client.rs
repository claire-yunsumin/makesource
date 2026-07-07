//! ComfyUI API 클라이언트 (TAD §6).
//!
//! `POST /prompt` → prompt_id, WebSocket(`/ws`)으로 진행률·완료 수신,
//! `POST /interrupt`로 취소. WS 메시지 해석은 순수 함수(`parse_ws_message`)로
//! 분리해 유닛 테스트한다.

use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;

use crate::error::AppError;

/// ComfyUI가 완료 시 알려주는 출력 이미지 위치.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OutputImage {
    pub filename: String,
    #[serde(default)]
    pub subfolder: String,
    #[serde(rename = "type", default)]
    pub folder_type: String,
}

/// WS 메시지를 앱 관점 이벤트로 해석한 결과.
#[derive(Debug, Clone, PartialEq)]
pub enum WsEvent {
    /// KSampler 진행률 (value/max)
    Progress { value: u32, max: u32 },
    /// 해당 prompt 실행 완료 (executing.node == null)
    Done { prompt_id: String },
    /// 노드 실행 결과에 이미지 출력 포함
    Images(Vec<OutputImage>),
    /// 실행 에러
    ExecutionError { message: String },
    /// 관심 없는 메시지
    Other,
}

/// ComfyUI WS 텍스트 메시지 1건을 해석한다 (순수 함수).
pub fn parse_ws_message(text: &str) -> WsEvent {
    let Ok(msg) = serde_json::from_str::<Value>(text) else {
        return WsEvent::Other;
    };
    let data = &msg["data"];
    match msg["type"].as_str() {
        Some("progress") => {
            let value = data["value"].as_u64().unwrap_or(0) as u32;
            let max = data["max"].as_u64().unwrap_or(1).max(1) as u32;
            WsEvent::Progress { value, max }
        }
        Some("executing") => {
            // node가 null이면 해당 prompt 실행 종료
            if data["node"].is_null() {
                if let Some(pid) = data["prompt_id"].as_str() {
                    return WsEvent::Done {
                        prompt_id: pid.to_string(),
                    };
                }
            }
            WsEvent::Other
        }
        Some("executed") => {
            match serde_json::from_value::<Vec<OutputImage>>(data["output"]["images"].clone()) {
                Ok(images) if !images.is_empty() => WsEvent::Images(images),
                _ => WsEvent::Other,
            }
        }
        Some("execution_error") => WsEvent::ExecutionError {
            message: data["exception_message"]
                .as_str()
                .unwrap_or("unknown execution error")
                .to_string(),
        },
        _ => WsEvent::Other,
    }
}

#[derive(Debug, Deserialize)]
struct PromptResponse {
    prompt_id: String,
}

/// 워크플로 제출. prompt_id 반환.
pub async fn post_prompt(
    client: &reqwest::Client,
    base_url: &str,
    payload: &Value,
) -> Result<String, AppError> {
    let resp = client
        .post(format!("{base_url}/prompt"))
        .json(payload)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::with_detail(
            "E_ENGINE_REJECT",
            "엔진이 작업을 받아들이지 않았어요.",
            format!("HTTP {status}: {body}"),
        ));
    }
    let parsed: PromptResponse = resp.json().await?;
    Ok(parsed.prompt_id)
}

/// 실행 중인 작업 중단 (취소 — TAD §5 generate_cancel).
pub async fn interrupt(client: &reqwest::Client, base_url: &str) -> Result<(), AppError> {
    client
        .post(format!("{base_url}/interrupt"))
        .send()
        .await?
        .error_for_status()
        .map_err(AppError::from)?;
    Ok(())
}

/// WS로 특정 prompt의 진행률·완료를 추적한다.
/// 완료 시 수집된 출력 이미지 목록을 반환. 취소 토큰이 켜지면 interrupt 후 E_CANCELED.
pub async fn track_progress(
    base_url: &str,
    client_id: &str,
    prompt_id: &str,
    cancel: &tokio::sync::watch::Receiver<bool>,
    mut on_progress: impl FnMut(u32, u32),
) -> Result<Vec<OutputImage>, AppError> {
    let ws_url = format!(
        "{}/ws?clientId={client_id}",
        base_url.replacen("http", "ws", 1)
    );
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .map_err(|e| AppError::with_detail("E_ENGINE_WS", "엔진 연결이 끊어졌어요.", e))?;

    let http = crate::engine::shared_http();
    let mut images: Vec<OutputImage> = Vec::new();
    let mut cancel = cancel.clone();

    loop {
        tokio::select! {
            _ = cancel.changed() => {
                if *cancel.borrow() {
                    let _ = interrupt(http, base_url).await;
                    return Err(AppError::new("E_CANCELED", "생성을 취소했어요."));
                }
            }
            msg = ws.next() => {
                let Some(msg) = msg else {
                    return Err(AppError::new("E_ENGINE_WS", "엔진 연결이 끊어졌어요."));
                };
                let msg = msg.map_err(|e| {
                    AppError::with_detail("E_ENGINE_WS", "엔진 연결이 끊어졌어요.", e)
                })?;
                let Message::Text(text) = msg else { continue };
                match parse_ws_message(&text) {
                    WsEvent::Progress { value, max } => on_progress(value, max),
                    WsEvent::Images(mut batch) => images.append(&mut batch),
                    WsEvent::Done { prompt_id: done_id } if done_id == prompt_id => {
                        return Ok(images);
                    }
                    WsEvent::ExecutionError { message } => {
                        return Err(AppError::with_detail(
                            "E_ENGINE_EXEC",
                            "이미지를 만들다가 문제가 생겼어요.",
                            message,
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress() {
        let ev = parse_ws_message(r#"{"type":"progress","data":{"value":7,"max":28}}"#);
        assert_eq!(ev, WsEvent::Progress { value: 7, max: 28 });
    }

    #[test]
    fn parses_done_on_null_node() {
        let ev =
            parse_ws_message(r#"{"type":"executing","data":{"node":null,"prompt_id":"abc-123"}}"#);
        assert_eq!(
            ev,
            WsEvent::Done {
                prompt_id: "abc-123".into()
            }
        );
        // node가 있으면 계속 실행 중
        let ev = parse_ws_message(r#"{"type":"executing","data":{"node":"3","prompt_id":"x"}}"#);
        assert_eq!(ev, WsEvent::Other);
    }

    #[test]
    fn parses_executed_images() {
        let ev = parse_ws_message(
            r#"{"type":"executed","data":{"node":"9","output":{"images":[
                {"filename":"localbrush_00001_.png","subfolder":"","type":"output"}
            ]}}}"#,
        );
        match ev {
            WsEvent::Images(images) => {
                assert_eq!(images.len(), 1);
                assert_eq!(images[0].filename, "localbrush_00001_.png");
                assert_eq!(images[0].folder_type, "output");
            }
            other => panic!("Images여야 함: {other:?}"),
        }
    }

    #[test]
    fn parses_execution_error() {
        let ev = parse_ws_message(
            r#"{"type":"execution_error","data":{"exception_message":"CUDA out of memory"}}"#,
        );
        assert_eq!(
            ev,
            WsEvent::ExecutionError {
                message: "CUDA out of memory".into()
            }
        );
    }

    #[test]
    fn tolerates_garbage_and_unknown_types() {
        assert_eq!(parse_ws_message("not json"), WsEvent::Other);
        assert_eq!(
            parse_ws_message(r#"{"type":"status","data":{}}"#),
            WsEvent::Other
        );
        assert_eq!(
            parse_ws_message(r#"{"type":"executed","data":{"output":{}}}"#),
            WsEvent::Other
        );
    }
}
