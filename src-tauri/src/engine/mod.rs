//! ComfyUI 프로세스 매니저 (TAD §6).
//!
//! 기동: `runtime/venv/bin/python runtime/comfyui/main.py --listen 127.0.0.1 --port 8188`
//! 헬스체크: `/system_stats` 폴링. 앱 종료 시 kill.
//! 크래시 감지 시 1회 자동 재시작 (그 이상은 사용자에게 맡김 — OOM 폴백은 T1.5).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::Mutex;

use crate::error::AppError;

/// 엔진 실행 구성. data_root에서 파생 (TAD §3/§6).
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub python: PathBuf,
    pub main_py: PathBuf,
    pub port: u16,
}

impl EngineConfig {
    pub const DEFAULT_PORT: u16 = 8188;

    pub fn from_data_root(data_root: &Path) -> Self {
        Self {
            python: data_root.join("runtime/venv/bin/python"),
            main_py: data_root.join("runtime/comfyui/main.py"),
            port: Self::DEFAULT_PORT,
        }
    }

    /// ComfyUI 기동 인자 (TAD §6 명세 그대로).
    pub fn spawn_spec(&self) -> SpawnSpec {
        SpawnSpec {
            program: self.python.clone(),
            args: vec![
                self.main_py.to_string_lossy().into_owned(),
                "--listen".into(),
                "127.0.0.1".into(),
                "--port".into(),
                self.port.to_string(),
            ],
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// 헬스체크 엔드포인트 (TAD §6: /system_stats).
    pub fn health_url(&self) -> String {
        format!("{}/system_stats", self.base_url())
    }

    /// 부트스트랩 산출물이 있어야 기동 가능.
    pub fn is_installed(&self) -> bool {
        self.python.exists() && self.main_py.exists()
    }
}

/// 실행할 프로세스 사양. 테스트에서는 더미 프로세스로 대체해
/// 크래시 감지·재시작 로직만 독립 검증한다.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
}

/// `engine_health` 응답 (TAD §5).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineHealth {
    pub running: bool,
    pub model_loaded: bool,
}

struct Inner {
    child: Option<tokio::process::Child>,
    /// 크래시 시 남은 자동 재시작 횟수 (TAD §6: 1회)
    restarts_left: u8,
    /// 의도된 종료 중이면 재시작하지 않음
    shutting_down: bool,
}

/// 엔진 수퍼바이저. 프로세스 생명주기(기동/감시/재시작/종료)를 소유한다.
pub struct EngineManager {
    spec: SpawnSpec,
    log_path: Option<PathBuf>,
    inner: Arc<Mutex<Inner>>,
}

impl EngineManager {
    const RESTART_LIMIT: u8 = 1;

    pub fn new(spec: SpawnSpec, log_path: Option<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            spec,
            log_path,
            inner: Arc::new(Mutex::new(Inner {
                child: None,
                restarts_left: Self::RESTART_LIMIT,
                shutting_down: false,
            })),
        })
    }

    /// logs/engine.log에 한 줄 기록.
    fn log(&self, line: &str) {
        use std::io::Write;
        if let Some(path) = &self.log_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(f, "{line}");
            }
        }
    }

    fn spawn_child(&self) -> Result<tokio::process::Child, AppError> {
        // 엔진 stdout/stderr는 로그 파일로 (없으면 무시)
        let (out, err) = match &self.log_path {
            Some(path) => {
                let open = || {
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(path)
                };
                match (open(), open()) {
                    (Ok(a), Ok(b)) => (Stdio::from(a), Stdio::from(b)),
                    _ => (Stdio::null(), Stdio::null()),
                }
            }
            None => (Stdio::null(), Stdio::null()),
        };

        tokio::process::Command::new(&self.spec.program)
            .args(&self.spec.args)
            .stdout(out)
            .stderr(err)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                AppError::with_detail(
                    "E_ENGINE_SPAWN",
                    "생성 엔진을 시작하지 못했어요.",
                    format!("{}: {e}", self.spec.program.display()),
                )
            })
    }

    /// 엔진 기동 + 크래시 감시 태스크 시작. 이미 실행 중이면 no-op.
    pub async fn start(self: &Arc<Self>) -> Result<(), AppError> {
        {
            let mut inner = self.inner.lock().await;
            if let Some(child) = inner.child.as_mut() {
                if matches!(child.try_wait(), Ok(None)) {
                    return Ok(()); // 이미 실행 중
                }
            }
            inner.shutting_down = false;
            let child = self.spawn_child()?;
            self.log(&format!("engine 시작: pid={:?}", child.id()));
            inner.child = Some(child);
        }
        self.clone().watch();
        Ok(())
    }

    /// 크래시 감시: 주기적으로 try_wait를 폴링해 비정상 종료 시 1회 재시작.
    /// (child 소유권을 유지해 is_process_running/pid가 계속 동작하도록 폴링 방식 사용)
    fn watch(self: Arc<Self>) {
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                let mut inner = self.inner.lock().await;
                if inner.shutting_down {
                    self.log("engine 감시 종료 (요청됨)");
                    return;
                }
                let exit_status = match inner.child.as_mut() {
                    None => return, // shutdown 등으로 정리됨
                    Some(child) => match child.try_wait() {
                        Ok(None) => continue, // 정상 실행 중
                        Ok(Some(status)) => format!("{status}"),
                        Err(e) => format!("wait 실패: {e}"),
                    },
                };

                // 비정상 종료 감지
                if inner.restarts_left == 0 {
                    self.log(&format!(
                        "engine 크래시 (status={exit_status}) — 재시작 한도 초과, 중단"
                    ));
                    inner.child = None;
                    return;
                }
                inner.restarts_left -= 1;
                self.log(&format!(
                    "engine 크래시 감지 (status={exit_status}) — 자동 재시작 시도"
                ));
                match self.spawn_child() {
                    Ok(child) => {
                        self.log(&format!("engine 재시작: pid={:?}", child.id()));
                        inner.child = Some(child);
                        // 루프 계속 → 재시작한 프로세스도 감시
                    }
                    Err(e) => {
                        self.log(&format!("engine 재시작 실패: {e}"));
                        inner.child = None;
                        return;
                    }
                }
            }
        });
    }

    /// 프로세스 생존 여부 (헬스 HTTP와 별개의 1차 판정).
    pub async fn is_process_running(&self) -> bool {
        let mut inner = self.inner.lock().await;
        match inner.child.as_mut() {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => false,
        }
    }

    /// 현재 pid (테스트/디버그용).
    pub async fn pid(&self) -> Option<u32> {
        self.inner.lock().await.child.as_ref().and_then(|c| c.id())
    }

    /// 의도된 종료 (앱 종료 시). 재시작하지 않는다.
    pub async fn shutdown(&self) {
        let mut inner = self.inner.lock().await;
        inner.shutting_down = true;
        if let Some(child) = inner.child.as_mut() {
            let _ = child.start_kill();
        }
        inner.child = None;
    }
}

/// 헬스체크: 프로세스 생존 + `/system_stats` 응답 (TAD §6).
/// model_loaded는 첫 생성 워크플로 연동(T1.4)에서 채운다.
pub async fn check_health(
    manager: &EngineManager,
    client: &reqwest::Client,
    health_url: &str,
) -> EngineHealth {
    if !manager.is_process_running().await {
        return EngineHealth {
            running: false,
            model_loaded: false,
        };
    }
    let running = client
        .get(health_url)
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    EngineHealth {
        running,
        model_loaded: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn sleep_spec() -> SpawnSpec {
        // ComfyUI 대역: 오래 사는 무해한 프로세스
        SpawnSpec {
            program: PathBuf::from("/bin/sleep"),
            args: vec!["300".into()],
        }
    }

    /// SIGKILL로 크래시 시뮬레이션.
    fn kill9(pid: u32) {
        let _ = std::process::Command::new("/bin/kill")
            .args(["-9", &pid.to_string()])
            .status();
    }

    #[test]
    fn config_builds_tad_spawn_spec() {
        let cfg = EngineConfig::from_data_root(Path::new("/data/LocalBrush"));
        let spec = cfg.spawn_spec();
        assert_eq!(
            spec.program,
            Path::new("/data/LocalBrush/runtime/venv/bin/python")
        );
        assert_eq!(
            spec.args,
            vec![
                "/data/LocalBrush/runtime/comfyui/main.py",
                "--listen",
                "127.0.0.1",
                "--port",
                "8188"
            ]
        );
        assert_eq!(cfg.health_url(), "http://127.0.0.1:8188/system_stats");
    }

    #[test]
    fn is_installed_requires_bootstrap_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = EngineConfig::from_data_root(dir.path());
        assert!(!cfg.is_installed());
    }

    #[tokio::test]
    async fn start_is_idempotent_and_process_runs() {
        let mgr = EngineManager::new(sleep_spec(), None);
        mgr.start().await.unwrap();
        assert!(mgr.is_process_running().await);
        let pid = mgr.pid().await;

        // 두 번째 start는 no-op (같은 pid 유지)
        mgr.start().await.unwrap();
        assert_eq!(mgr.pid().await, pid);

        mgr.shutdown().await;
    }

    #[tokio::test]
    async fn crash_triggers_single_auto_restart() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("engine.log");
        let mgr = EngineManager::new(sleep_spec(), Some(log.clone()));
        mgr.start().await.unwrap();
        let pid1 = mgr.pid().await.unwrap();

        // 1차 크래시 시뮬레이션 → 자동 재시작 (새 pid)
        kill9(pid1);
        let mut restarted = false;
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if let Some(pid) = mgr.pid().await {
                if pid != pid1 {
                    restarted = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(restarted, "크래시 후 1회 자동 재시작해야 함");
        assert!(mgr.is_process_running().await);

        // 2차 크래시 → 한도 초과, 재시작하지 않음
        let pid2 = mgr.pid().await.unwrap();
        kill9(pid2);
        let mut stayed_dead = false;
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if !mgr.is_process_running().await {
                stayed_dead = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(stayed_dead, "재시작 한도 초과 시 중단돼야 함");
        // 한도 초과 후에도 재기동되지 않는지 한 번 더 확인
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!mgr.is_process_running().await);

        // 자동 복구 로그 확인 (AC)
        let log_text = std::fs::read_to_string(&log).unwrap();
        assert!(
            log_text.contains("자동 재시작"),
            "복구 로그가 남아야 함: {log_text}"
        );

        mgr.shutdown().await;
    }

    #[tokio::test]
    async fn shutdown_prevents_restart() {
        let mgr = EngineManager::new(sleep_spec(), None);
        mgr.start().await.unwrap();
        mgr.shutdown().await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!mgr.is_process_running().await, "종료 후 재시작하면 안 됨");
    }
}
