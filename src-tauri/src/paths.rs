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
}
