//! dev 전용 부트스트랩 러너 (UI 없이 T1.1/T1.4 AC 검증용).
//! 사용: cargo run --bin bootstrap_cli -- [light|standard]

use std::path::Path;
use std::sync::{Arc, Mutex};

use localbrush_lib::bootstrap::state::ModelProfile;
use localbrush_lib::bootstrap::{Bootstrapper, ProgressEvent};
use localbrush_lib::paths;

#[tokio::main]
async fn main() {
    let profile = match std::env::args().nth(1).as_deref() {
        Some("standard") => ModelProfile::Standard,
        _ => ModelProfile::Light,
    };

    let Ok(home) = std::env::var("HOME") else {
        eprintln!("HOME 환경변수를 찾을 수 없습니다");
        std::process::exit(1);
    };
    let base = format!("{home}/Library/Application Support");
    let root = paths::app_data_root(Path::new(&base));
    println!("데이터 루트: {} / 프로파일: {profile:?}", root.display());

    // 진행 출력 (다운로드 스팸 방지: 1% 단위로만)
    let last_pct = Mutex::new(-1i32);
    let emit = Arc::new(move |ev: ProgressEvent| {
        let pct = (ev.progress * 100.0) as i32;
        let mut last = match last_pct.lock() {
            Ok(v) => v,
            Err(_) => return,
        };
        if pct != *last || ev.error.is_some() {
            *last = pct;
            println!(
                "[{:>3}%] {:?} — {}{}",
                pct,
                ev.step,
                ev.message,
                match &ev.error {
                    Some(code) => format!(" (에러: {code})"),
                    None => String::new(),
                }
            );
        }
    });

    match Bootstrapper::new(root).run(profile, emit).await {
        Ok(()) => println!("== READY =="),
        Err(e) => {
            eprintln!("부트스트랩 실패: {e}");
            std::process::exit(1);
        }
    }
}
