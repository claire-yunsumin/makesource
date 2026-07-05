//! dev 전용 생성 E2E 러너 (T1.4 AC "CLI 수준 테스트" 검증용).
//! 엔진을 직접 기동하고 command와 동일한 코드 경로(run_generation)를 실행한다.
//!
//! 사용: cargo run --bin generate_cli -- "<키워드>" [count] [--cancel-after <초>]

use std::path::Path;
use std::sync::Arc;

use localbrush_lib::db::Db;
use localbrush_lib::engine::generation::{run_generation, GenerateRequest};
use localbrush_lib::engine::{check_health, EngineConfig, EngineManager};
use localbrush_lib::paths;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let keyword = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "통나무집".to_string());
    let count: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let cancel_after: Option<u64> = args
        .iter()
        .position(|a| a == "--cancel-after")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok());

    let Ok(home) = std::env::var("HOME") else {
        eprintln!("HOME 환경변수를 찾을 수 없습니다");
        std::process::exit(1);
    };
    let root = paths::app_data_root(Path::new(&format!("{home}/Library/Application Support")));
    let config = EngineConfig::from_data_root(&root);
    if !config.is_installed() {
        eprintln!("부트스트랩이 완료되지 않았습니다: {}", root.display());
        std::process::exit(1);
    }
    if let Err(e) = config.ensure_runtime_dirs() {
        eprintln!("엔진 런타임 폴더 준비 실패: {e}");
        std::process::exit(1);
    }

    // DB 연결
    let db = match Db::connect(&paths::db_path(&root)).await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("DB 연결 실패: {e}");
            std::process::exit(1);
        }
    };

    // 엔진 기동 + 헬스 대기
    let manager = EngineManager::new(
        config.spawn_spec(),
        Some(root.join("logs/engine.log")),
        Some(root.join("runtime/engine.pid")),
    );
    if let Err(e) = manager.start().await {
        eprintln!("엔진 기동 실패: {e}");
        std::process::exit(1);
    }
    let http = reqwest::Client::new();
    println!("엔진 기동 중… (/system_stats 대기)");
    let mut healthy = false;
    for _ in 0..120 {
        let h = check_health(&manager, &http, &config.health_url()).await;
        if h.running {
            healthy = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    if !healthy {
        eprintln!("엔진 헬스체크 실패 (120초 초과) — logs/engine.log 확인");
        manager.shutdown().await;
        std::process::exit(1);
    }
    println!("engine_health: running=true ✅ (T1.2 수동 AC)");

    // 취소 채널 (+옵션: N초 후 자동 취소)
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    if let Some(secs) = cancel_after {
        let tx = cancel_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            println!("\n>> {secs}초 경과 — 취소 요청");
            let _ = tx.send(true);
        });
    }

    let req = GenerateRequest {
        preset_id: "storybook".to_string(),
        keyword,
        count,
        size: Some((1024, 1024)),
        seed: None,
    };
    println!(
        "생성 시작: preset=storybook, keyword={:?}, count={count}",
        req.keyword
    );

    let job_id = uuid::Uuid::new_v4().to_string();
    let last = Arc::new(std::sync::Mutex::new(-1i32));
    let result = run_generation(&job_id, &root, &config.base_url(), &db, &req, &cancel_rx, {
        let last = last.clone();
        move |p| {
            let pct = (p * 100.0) as i32;
            if let Ok(mut l) = last.lock() {
                if pct / 5 != *l / 5 {
                    *l = pct;
                    println!("진행 {pct}%");
                }
            }
        }
    })
    .await;

    manager.shutdown().await;

    match result {
        Ok(done) => {
            println!("== 완료 ==");
            for p in &done.image_paths {
                println!("이미지: {} ({})", p, root.join(p).display());
            }
        }
        Err(e) => {
            eprintln!("결과: {} — {}", e.code, e.message);
            if e.code != "E_CANCELED" {
                std::process::exit(1);
            }
            println!("(취소 동작 확인됨)");
        }
    }
}
