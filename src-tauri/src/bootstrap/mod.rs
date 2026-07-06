//! 최초 실행 설치기 (TAD §7).
//!
//! 상태 머신을 단계별로 실행하고, 각 단계 완료 시 상태 파일에 기록해
//! 중단 후 재실행 시 이어서 진행한다. 진행 상황은 emit 콜백으로 밖에 알리고
//! (command 계층에서 `bootstrap://progress` 이벤트로 전달), 전 과정을
//! `logs/bootstrap.log`에 남긴다.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use serde::Serialize;

use crate::error::AppError;

pub mod download;
pub mod models;
pub mod state;

use state::{BootstrapState, ModelProfile, Step};

/// `bootstrap://progress` 이벤트 페이로드.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub step: Step,
    /// 전체 진행률 0.0~1.0
    pub progress: f64,
    /// 사용자용 한국어 상태 문구 (04 §6 톤)
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

const COMFYUI_REPO: &str = "https://github.com/comfyanonymous/ComfyUI.git";

pub struct Bootstrapper {
    data_root: PathBuf,
}

impl Bootstrapper {
    pub fn new(data_root: PathBuf) -> Self {
        Self { data_root }
    }

    pub fn state_path(&self) -> PathBuf {
        self.data_root.join("bootstrap.state.json")
    }

    fn uv_bin(&self) -> PathBuf {
        self.data_root.join("runtime/uv/uv")
    }

    fn venv_python(&self) -> PathBuf {
        self.data_root.join("runtime/venv/bin/python")
    }

    fn comfyui_dir(&self) -> PathBuf {
        self.data_root.join("runtime/comfyui")
    }

    /// logs/bootstrap.log에 한 줄 기록 (실패해도 설치는 계속).
    fn log(&self, line: &str) {
        let log_dir = self.data_root.join("logs");
        let _ = std::fs::create_dir_all(&log_dir);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("bootstrap.log"))
        {
            let _ = writeln!(f, "{line}");
        }
    }

    /// 서브프로세스 실행, stdout/stderr를 로그로. 실패 시 AppError.
    async fn run_cmd(
        &self,
        program: &Path,
        args: &[&str],
        cwd: Option<&Path>,
    ) -> Result<(), AppError> {
        self.log(&format!("$ {} {}", program.display(), args.join(" ")));
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let out = cmd.output().await.map_err(|e| {
            AppError::with_detail(
                "E_BOOTSTRAP_CMD",
                &format!("설치 명령을 실행하지 못했어요: {}", program.display()),
                e,
            )
        })?;
        self.log(&String::from_utf8_lossy(&out.stdout));
        self.log(&String::from_utf8_lossy(&out.stderr));
        if !out.status.success() {
            return Err(AppError::with_detail(
                "E_BOOTSTRAP_CMD",
                "설치 단계가 실패했어요. 로그를 확인해 주세요.",
                String::from_utf8_lossy(&out.stderr).into_owned(),
            ));
        }
        Ok(())
    }

    /// 상태 머신 실행. 저장된 지점부터 Ready까지 진행.
    pub async fn run(
        &self,
        profile: ModelProfile,
        emit: Arc<dyn Fn(ProgressEvent) + Send + Sync>,
    ) -> Result<(), AppError> {
        let state_path = self.state_path();
        let mut st = BootstrapState::load(&state_path);
        // 재개 시에는 이전 실행의 프로파일을 유지 (모델 세트가 갈리면 안 됨)
        let profile = st.model_profile.unwrap_or(profile);
        st.model_profile = Some(profile);
        st.save(&state_path)?;

        self.log(&format!(
            "== bootstrap 시작: step={:?}, profile={profile:?}",
            st.step
        ));

        while !st.is_ready() {
            let step = st.step;
            emit(ProgressEvent {
                step,
                progress: step.base_progress(),
                message: step_message(step).to_string(),
                error: None,
            });

            let result = match step {
                Step::Check => self.step_check().await,
                Step::InstallPython => self.step_install_python().await,
                Step::CloneComfyui => self.step_clone_comfyui().await,
                Step::PipInstall => self.step_pip_install().await,
                Step::DownloadModels => self.step_download_models(profile, emit.clone()).await,
                Step::Warmup => self.step_warmup().await,
                Step::Ready => Ok(()),
            };

            if let Err(err) = result {
                self.log(&format!("!! {step:?} 실패: {err}"));
                emit(ProgressEvent {
                    step,
                    progress: step.base_progress(),
                    message: err.message.clone(),
                    error: Some(err.code.clone()),
                });
                return Err(err);
            }

            // 단계 완료 → 다음 단계를 중단 지점으로 기록 (재개 가능성)
            if let Some(next) = step.next() {
                st.step = next;
                st.save(&state_path)?;
            }
            self.log(&format!("== {step:?} 완료"));
        }

        emit(ProgressEvent {
            step: Step::Ready,
            progress: 1.0,
            message: "준비가 끝났어요. 이제 이미지를 만들 수 있어요.".to_string(),
            error: None,
        });
        Ok(())
    }

    /// check: 폴더 구조 생성(TAD §3) + git 존재 확인.
    async fn step_check(&self) -> Result<(), AppError> {
        for dir in [
            "models/checkpoints",
            "models/loras",
            "models/ipadapter",
            "models/clip_vision",
            "runtime",
            "outputs",
            "training",
            "logs",
            // ComfyUI --base-directory가 요구하는 폴더 (없으면 기동 크래시 — 실측)
            "custom_nodes",
            "input",
            "output",
            "user",
        ] {
            std::fs::create_dir_all(self.data_root.join(dir))?;
        }
        // ComfyUI clone에 git 필요 (macOS: Xcode CLT)
        self.run_cmd(Path::new("/usr/bin/git"), &["--version"], None)
            .await
            .map_err(|e| {
                AppError::with_detail(
                    "E_GIT_MISSING",
                    "git을 찾을 수 없어요. Xcode 명령줄 도구를 설치해 주세요.",
                    e,
                )
            })
    }

    /// install_python: uv 바이너리 확보(GitHub 릴리스) → uv venv (파이썬 자동 다운로드).
    async fn step_install_python(&self) -> Result<(), AppError> {
        if !self.uv_bin().exists() {
            let arch = match std::env::consts::ARCH {
                "aarch64" => "aarch64",
                _ => "x86_64",
            };
            let tar_name = format!("uv-{arch}-apple-darwin");
            let url = format!(
                "https://github.com/astral-sh/uv/releases/latest/download/{tar_name}.tar.gz"
            );
            let tar_dest = self.data_root.join("runtime/uv.tar.gz");
            let client = reqwest::Client::new();
            download::download_resumable(&client, &url, &tar_dest, Box::new(|_, _| {})).await?;

            let uv_dir = self.data_root.join("runtime/uv");
            std::fs::create_dir_all(&uv_dir)?;
            self.run_cmd(
                Path::new("/usr/bin/tar"),
                &[
                    "-xzf",
                    &tar_dest.to_string_lossy(),
                    "-C",
                    &uv_dir.to_string_lossy(),
                    "--strip-components",
                    "1",
                ],
                None,
            )
            .await?;
            let _ = std::fs::remove_file(&tar_dest);
        }

        if !self.venv_python().exists() {
            let venv = self.data_root.join("runtime/venv");
            self.run_cmd(
                &self.uv_bin(),
                &["venv", &venv.to_string_lossy(), "--python", "3.11"],
                None,
            )
            .await?;
        }
        Ok(())
    }

    /// clone_comfyui: 얕은 클론 (이미 있으면 스킵 — 멱등).
    async fn step_clone_comfyui(&self) -> Result<(), AppError> {
        if self.comfyui_dir().join(".git").exists() {
            return Ok(());
        }
        self.run_cmd(
            Path::new("/usr/bin/git"),
            &[
                "clone",
                "--depth",
                "1",
                COMFYUI_REPO,
                &self.comfyui_dir().to_string_lossy(),
            ],
            None,
        )
        .await
    }

    /// pip_install: ComfyUI requirements + 앱 파이썬 도구를 venv에 설치 (uv pip).
    async fn step_pip_install(&self) -> Result<(), AppError> {
        let req = self.comfyui_dir().join("requirements.txt");
        self.run_cmd(
            &self.uv_bin(),
            &[
                "pip",
                "install",
                "-r",
                &req.to_string_lossy(),
                "--python",
                &self.venv_python().to_string_lossy(),
            ],
            None,
        )
        .await?;
        // 앱 파이썬 도구: 한→영 변환(T2.3b) + 배경 제거(T2.4b).
        // 모델은 download_models 단계에서 받는다
        self.run_cmd(
            &self.uv_bin(),
            &[
                "pip",
                "install",
                "argostranslate",
                "rembg",
                "--python",
                &self.venv_python().to_string_lossy(),
            ],
            None,
        )
        .await
    }

    /// download_models: 프로파일별 모델을 HF에서 이어받기 다운로드.
    async fn step_download_models(
        &self,
        profile: ModelProfile,
        emit: Arc<dyn Fn(ProgressEvent) + Send + Sync>,
    ) -> Result<(), AppError> {
        let specs = models::models_for(profile);
        let total_bytes: u64 = specs.iter().map(|m| m.approx_bytes).sum();
        let step_base = Step::DownloadModels.base_progress();
        let step_span = Step::Warmup.base_progress() - step_base;

        let client = reqwest::Client::new();
        let mut done_bytes: u64 = 0;
        for spec in specs {
            let dest = self.data_root.join(spec.dest_rel);
            self.log(&format!("모델 다운로드: {} -> {}", spec.url, spec.dest_rel));

            let emit2 = emit.clone();
            let base = done_bytes;
            let approx = spec.approx_bytes;
            download::download_resumable(
                &client,
                spec.url,
                &dest,
                Box::new(move |written, total| {
                    let file_total = total.unwrap_or(approx).max(1);
                    let frac = (base as f64 + written.min(file_total) as f64) / total_bytes as f64;
                    emit2(ProgressEvent {
                        step: Step::DownloadModels,
                        progress: step_base + frac.min(1.0) * step_span,
                        message: "모델을 내려받는 중이에요.".to_string(),
                        error: None,
                    });
                }),
            )
            .await?;
            done_bytes += spec.approx_bytes;
        }
        Ok(())
    }

    /// warmup: venv 파이썬으로 torch 임포트 확인 (엔진 실기동은 T1.2).
    async fn step_warmup(&self) -> Result<(), AppError> {
        self.run_cmd(
            &self.venv_python(),
            &["-c", "import torch; print(torch.__version__)"],
            None,
        )
        .await
    }
}

/// 단계별 사용자 문구 (04 §6 톤).
pub fn step_message(step: Step) -> &'static str {
    match step {
        Step::Check => "환경을 확인하는 중이에요.",
        Step::InstallPython => "파이썬 환경을 준비하는 중이에요.",
        Step::CloneComfyui => "생성 엔진을 내려받는 중이에요.",
        Step::PipInstall => "엔진 구성 요소를 설치하는 중이에요.",
        Step::DownloadModels => "모델을 내려받는 중이에요.",
        Step::Warmup => "마무리 점검 중이에요.",
        Step::Ready => "준비가 끝났어요.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_event_serializes_contract_shape() {
        let ev = ProgressEvent {
            step: Step::DownloadModels,
            progress: 0.72,
            message: "모델을 내려받는 중이에요.".to_string(),
            error: None,
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["step"], "download_models");
        assert_eq!(json["progress"], 0.72);
        assert!(json.get("error").is_none());
    }

    #[test]
    fn every_step_has_korean_message() {
        for step in Step::ORDER {
            assert!(!step_message(step).is_empty());
        }
    }

    #[tokio::test]
    async fn check_step_creates_tad_folder_layout() {
        let dir = tempfile::tempdir().unwrap();
        let b = Bootstrapper::new(dir.path().to_path_buf());
        b.step_check().await.unwrap();
        for sub in [
            "models/checkpoints",
            "models/loras",
            "runtime",
            "outputs",
            "logs",
        ] {
            assert!(dir.path().join(sub).is_dir(), "{sub} 폴더가 생성돼야 함");
        }
    }

    /// 네트워크 의존 통합 검증 (수동 실행 전용):
    /// check → install_python(uv+venv) → clone_comfyui 실단계 + 상태 파일 재개 기록.
    /// 실행: cargo test bootstrap_real_steps -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "네트워크·수 분 소요 — 로컬 수동 검증용 (모델 다운로드는 제외)"]
    async fn bootstrap_real_steps_through_clone() {
        let dir = tempfile::tempdir().unwrap();
        let b = Bootstrapper::new(dir.path().to_path_buf());

        b.step_check().await.unwrap();
        b.step_install_python().await.unwrap();
        assert!(b.uv_bin().exists(), "uv 바이너리가 설치돼야 함");
        assert!(b.venv_python().exists(), "venv 파이썬이 생성돼야 함");

        b.step_clone_comfyui().await.unwrap();
        assert!(
            b.comfyui_dir().join("main.py").exists(),
            "ComfyUI main.py 존재"
        );
        // 멱등성: 재실행해도 성공
        b.step_clone_comfyui().await.unwrap();

        // 재개 기록 시뮬레이션: pip_install 지점 저장 → 재로드
        let st = BootstrapState {
            step: Step::PipInstall,
            model_profile: Some(ModelProfile::Light),
        };
        st.save(&b.state_path()).unwrap();
        assert_eq!(BootstrapState::load(&b.state_path()), st);

        // 로그 파일 생성 확인 (TAD §7: logs/bootstrap.log)
        assert!(dir.path().join("logs/bootstrap.log").exists());
    }
}
