//! 앱 공통 에러 (TAD §5/§9).
//!
//! 모든 Tauri command는 `Result<T, AppError>`를 반환한다.
//! `message`는 사용자용 한국어(04 §6 톤), `detail`에 원문/원인을 담는다.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{code}: {message}")]
pub struct AppError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            detail: None,
        }
    }

    pub fn with_detail(code: &str, message: &str, detail: impl ToString) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            detail: Some(detail.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::with_detail("E_IO", "파일 작업 중 문제가 생겼어요.", err)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::with_detail("E_DB", "저장소에 접근하지 못했어요.", err)
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::with_detail("E_NETWORK", "네트워크 연결에 문제가 있어요.", err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_contract_shape() {
        let err = AppError::with_detail("E_X", "문제가 생겼어요.", "raw cause");
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "E_X");
        assert_eq!(json["message"], "문제가 생겼어요.");
        assert_eq!(json["detail"], "raw cause");
    }

    #[test]
    fn detail_omitted_when_none() {
        let json = serde_json::to_value(AppError::new("E_X", "m")).unwrap();
        assert!(json.get("detail").is_none());
    }
}
