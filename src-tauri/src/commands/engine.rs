//! engine_health (TAD §5).

use std::sync::Arc;

use tauri::State;

use crate::engine::{check_health, EngineConfig, EngineHealth, EngineManager};
use crate::error::AppError;

/// 관리 상태: 엔진 수퍼바이저 + 구성 + HTTP 클라이언트.
pub struct Engine {
    pub manager: Arc<EngineManager>,
    pub config: EngineConfig,
    pub client: reqwest::Client,
}

#[tauri::command]
pub async fn engine_health(state: State<'_, Engine>) -> Result<EngineHealth, AppError> {
    Ok(check_health(&state.manager, &state.client, &state.config.health_url()).await)
}
