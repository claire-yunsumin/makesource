//! kohya sd-scripts 선택 설치 (TAD §2 `training/`, T6.1).
//!
//! LoRA 학습(M6)은 이미지 생성과 무관한 무거운 의존성이라 메인 부트스트랩
//! (§7 7단계)에는 넣지 않고, 학습 화면 첫 사용 시 지연 설치한다. venv/uv는
//! 메인 부트스트랩이 이미 준비해둔 것을 그대로 재사용한다(Bootstrapper의
//! run_cmd/uv_bin/venv_python에 위임 — 로그도 같은 bootstrap.log에 쌓임).

use std::path::{Path, PathBuf};

use crate::bootstrap::Bootstrapper;
use crate::error::AppError;

const SD_SCRIPTS_REPO: &str = "https://github.com/kohya-ss/sd-scripts.git";

pub fn kohya_dir(data_root: &Path) -> PathBuf {
    data_root.join("runtime/kohya")
}

/// requirements 설치까지 끝나야 "설치됨"으로 본다 — 클론만 되고 pip가 중간에
/// 실패한 상태를 "설치됨"으로 오판하지 않기 위한 마커.
fn installed_marker(data_root: &Path) -> PathBuf {
    kohya_dir(data_root).join(".installed")
}

pub fn is_kohya_installed(data_root: &Path) -> bool {
    installed_marker(data_root).exists()
}

/// 지연 설치: sd-scripts 얕은 클론 + venv에 requirements 설치. 멱등(이미
/// 설치됐으면 즉시 반환, 클론은 `.git`이 있으면 스킵).
pub async fn ensure_kohya_installed(data_root: &Path) -> Result<(), AppError> {
    if is_kohya_installed(data_root) {
        return Ok(());
    }
    let b = Bootstrapper::new(data_root.to_path_buf());
    let dir = kohya_dir(data_root);
    if !dir.join(".git").exists() {
        b.run_cmd(
            Path::new("/usr/bin/git"),
            &[
                "clone",
                "--depth",
                "1",
                SD_SCRIPTS_REPO,
                &dir.to_string_lossy(),
            ],
            None,
        )
        .await?;
    }
    let req = dir.join("requirements.txt");
    b.run_cmd(
        &b.uv_bin(),
        &[
            "pip",
            "install",
            "-r",
            &req.to_string_lossy(),
            "--python",
            &b.venv_python().to_string_lossy(),
        ],
        None,
    )
    .await?;
    std::fs::write(installed_marker(data_root), b"ok")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_installed_when_marker_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_kohya_installed(dir.path()));
    }

    #[test]
    fn installed_once_marker_written() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(kohya_dir(dir.path())).unwrap();
        std::fs::write(installed_marker(dir.path()), b"ok").unwrap();
        assert!(is_kohya_installed(dir.path()));
    }

    #[tokio::test]
    async fn ensure_kohya_installed_is_noop_when_already_marked() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(kohya_dir(dir.path())).unwrap();
        std::fs::write(installed_marker(dir.path()), b"ok").unwrap();
        // 마커가 있으면 git/uv를 호출하지 않고 바로 성공 — 네트워크 없이도 통과해야 함
        ensure_kohya_installed(dir.path()).await.unwrap();
    }

    /// 네트워크 의존 통합 검증 (수동 실행 전용, bootstrap_real_steps_through_clone과 동일 취지).
    /// 실행: cargo test kohya_real_install -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "네트워크·수 분 소요 — 로컬 수동 검증용"]
    async fn kohya_real_install() {
        let dir = tempfile::tempdir().unwrap();
        ensure_kohya_installed(dir.path()).await.unwrap();
        assert!(is_kohya_installed(dir.path()));
        assert!(kohya_dir(dir.path()).join("requirements.txt").exists());
    }
}
