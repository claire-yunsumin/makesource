//! 앱 데이터 경로 규약 (TAD §3).
//!
//! 앱 데이터 루트는 `~/Library/Application Support/LocalBrush/`로 고정한다.
//! Tauri의 `app_data_dir()`(식별자 기반 `com.localbrush.app`)이 아니라
//! OS 데이터 폴더 + `LocalBrush`를 사용 — TAD 문서와 실제 경로를 일치시키기 위함.
//! 히스토리 이미지 등은 이 루트 기준 상대 경로로 저장한다 (CLAUDE.md 주의사항).

use std::path::{Path, PathBuf};

/// 앱 데이터 루트 폴더명 (TAD §3).
pub const APP_DATA_DIR_NAME: &str = "LocalBrush";

/// OS 데이터 폴더(base) 아래의 앱 데이터 루트를 계산한다.
/// base는 macOS에서 `~/Library/Application Support` (tauri `data_dir()`).
pub fn app_data_root(base: &Path) -> PathBuf {
    base.join(APP_DATA_DIR_NAME)
}

/// 앱 데이터 루트 아래 SQLite DB 경로 (TAD §3: `app.db`).
pub fn db_path(app_data_root: &Path) -> PathBuf {
    app_data_root.join("app.db")
}

/// 파일 이동: rename 시도 후 실패하면 copy+삭제 (T9.10, docs/11 §P7.2).
/// rename은 크로스 볼륨(사용자가 저장 위치를 외장 디스크로 옮긴 경우 등)에서
/// 실패한다 — F-5.2 저장 위치 변경을 대비한 폴백.
pub fn move_file(src: &Path, dst: &Path) -> std::io::Result<()> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(src, dst)?;
            std::fs::remove_file(src)
        }
    }
}

/// 내용이 다를 때만 파일을 쓴다 (T9.2, docs/11 §P1.7).
/// 내장 파이썬 스크립트가 호출마다 디스크에 재기록되는 것을 막는다.
pub fn write_if_changed(path: &Path, content: &str) -> std::io::Result<()> {
    if let Ok(existing) = std::fs::read_to_string(path) {
        if existing == content {
            return Ok(());
        }
    }
    std::fs::write(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_data_root_appends_localbrush() {
        let root = app_data_root(Path::new("/Users/x/Library/Application Support"));
        assert_eq!(
            root,
            Path::new("/Users/x/Library/Application Support/LocalBrush")
        );
    }

    #[test]
    fn db_path_is_app_db_under_root() {
        let root = Path::new("/data/LocalBrush");
        assert_eq!(db_path(root), Path::new("/data/LocalBrush/app.db"));
    }

    #[test]
    fn move_file_moves_and_removes_source() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.png");
        let dst = dir.path().join("out/b.png");
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::write(&src, b"img").unwrap();

        move_file(&src, &dst).unwrap();
        assert!(!src.exists());
        assert_eq!(std::fs::read(&dst).unwrap(), b"img");

        // 원본이 없으면 에러 전파
        assert!(move_file(&src, &dst).is_err());
    }

    #[test]
    fn write_if_changed_skips_identical_content() {
        use std::time::Duration;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("script.py");
        write_if_changed(&path, "print(1)").unwrap();
        let first_mtime = std::fs::metadata(&path).unwrap().modified().unwrap();

        std::thread::sleep(Duration::from_millis(20));
        // 같은 내용 → 쓰기 스킵 (mtime 불변)
        write_if_changed(&path, "print(1)").unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            first_mtime
        );

        // 다른 내용 → 갱신
        write_if_changed(&path, "print(2)").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "print(2)");
    }
}
