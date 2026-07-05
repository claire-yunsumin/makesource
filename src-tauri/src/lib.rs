//! LocalBrush Rust 백엔드 진입점.
//! command/engine/training/bootstrap/db/prompt 모듈은 해당 태스크에서 추가한다 (TAD §2).

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // CLAUDE.md 규칙 5: unwrap/expect는 테스트에서만. 여기선 명시적으로 처리.
    if let Err(err) = tauri::Builder::default().run(tauri::generate_context!()) {
        eprintln!("치명적: Tauri 앱을 실행하지 못했습니다: {err}");
        std::process::exit(1);
    }
}
