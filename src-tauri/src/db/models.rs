//! DB 행 모델 (TAD §3.1). serde 직렬화로 이후 Tauri command 응답에 재사용한다.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Generation {
    pub id: String,
    /// unix ms
    pub created_at: i64,
    pub image_path: String,
    pub thumb_path: String,
    pub keyword_ko: Option<String>,
    pub prompt_final: String,
    pub negative: Option<String>,
    pub preset_id: Option<String>,
    pub preset_version: Option<i64>,
    pub style_id: Option<String>,
    pub seed: i64,
    pub steps: Option<i64>,
    pub cfg: Option<f64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub model: Option<String>,
    pub favorite: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TrainingJob {
    pub id: String,
    pub style_id: String,
    /// queued|captioning|training|done|failed|canceled
    pub status: String,
    pub progress: f64,
    pub eta_seconds: Option<i64>,
    pub params_json: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}
