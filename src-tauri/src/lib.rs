//! LocalBrush Rust 백엔드 진입점.
//! command/engine/training/bootstrap/prompt 모듈은 해당 태스크에서 추가한다 (TAD §2).

use tauri::Manager;

pub mod db;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // CLAUDE.md 규칙 5: unwrap/expect는 테스트에서만. 여기선 명시적으로 처리.
    let result = tauri::Builder::default()
        .setup(|app| {
            // 앱 데이터 폴더에 app.db 생성 + 마이그레이션 (TAD §3)
            let db_path = app.path().app_data_dir()?.join("app.db");
            let db = tauri::async_runtime::block_on(db::Db::connect(&db_path))?;
            app.manage(db);
            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(err) = result {
        eprintln!("치명적: Tauri 앱을 실행하지 못했습니다: {err}");
        std::process::exit(1);
    }
}
