//! LocalBrush Rust 백엔드 진입점.
//! training/prompt 모듈은 해당 태스크에서 추가한다 (TAD §2).

use tauri::Manager;

pub mod bootstrap;
pub mod commands;
pub mod db;
pub mod engine;
pub mod error;
pub mod paths;
pub mod prompt;
pub mod styles;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // CLAUDE.md 규칙 5: unwrap/expect는 테스트에서만. 여기선 명시적으로 처리.
    let app = tauri::Builder::default()
        // Finder 드래그 아웃 (T3.4)
        .plugin(tauri_plugin_drag::init())
        // 프리셋 내보내기/가져오기 파일 선택 다이얼로그 (T5.3)
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 앱 데이터 루트(~/Library/Application Support/LocalBrush)에
            // app.db 생성 + 마이그레이션 (TAD §3)
            let data_root = paths::app_data_root(&app.path().data_dir()?);
            let db = tauri::async_runtime::block_on(db::Db::connect(&paths::db_path(&data_root)))?;
            app.manage(db);
            app.manage(commands::bootstrap::BootstrapJob::default());
            app.manage(commands::generate::GenJobs::default());
            app.manage(commands::training::KohyaInstallJob::default());

            // 엔진 수퍼바이저 (TAD §6). 부트스트랩 완료 상태면 즉시 기동.
            let config = engine::EngineConfig::from_data_root(&data_root);
            let manager = engine::EngineManager::new(
                config.spawn_spec(),
                Some(data_root.join("logs/engine.log")),
                Some(data_root.join("runtime/engine.pid")),
            );
            let engine_state = commands::engine::Engine {
                manager: manager.clone(),
                config: config.clone(),
                client: reqwest::Client::new(),
            };
            app.manage(engine_state);

            let bootstrap_ready = bootstrap::state::BootstrapState::load(
                &bootstrap::Bootstrapper::new(data_root).state_path(),
            )
            .is_ready();
            if bootstrap_ready && config.is_installed() {
                if let Err(e) = config.ensure_runtime_dirs() {
                    eprintln!("엔진 런타임 폴더 준비 실패: {e}");
                }
                tauri::async_runtime::block_on(async {
                    if let Err(e) = manager.start().await {
                        // 기동 실패는 치명적이지 않음 — engine_health가 false를 반환하고
                        // UI에서 재시작 유도 (04 §6)
                        eprintln!("엔진 자동 기동 실패: {e}");
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap::bootstrap_status,
            commands::bootstrap::bootstrap_run,
            commands::engine::engine_health,
            commands::generate::generate,
            commands::generate::generate_cancel,
            commands::presets::presets_get,
            commands::presets::presets_save,
            commands::presets::presets_export,
            commands::presets::presets_import,
            commands::training::kohya_install_status,
            commands::training::kohya_install_run,
            commands::translate::translate_keyword,
            commands::history::history_list,
            commands::history::history_toggle_favorite,
            commands::export::export_image,
            commands::essence::essence_create,
            commands::styles::styles_list,
            commands::styles::style_save,
            commands::styles::style_delete,
        ])
        .build(tauri::generate_context!());

    let app = match app {
        Ok(app) => app,
        Err(err) => {
            eprintln!("치명적: Tauri 앱을 초기화하지 못했습니다: {err}");
            std::process::exit(1);
        }
    };

    app.run(|app_handle, event| {
        // 앱 종료 시 엔진 서브프로세스 kill (TAD §6)
        if let tauri::RunEvent::Exit = event {
            if let Some(engine) = app_handle.try_state::<commands::engine::Engine>() {
                tauri::async_runtime::block_on(engine.manager.shutdown());
            }
        }
    });
}
