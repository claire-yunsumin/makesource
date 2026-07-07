//! 갤러리 썸네일 파이프라인 (T9.3, docs/11 §P2).
//!
//! 문제: 지금까지 `thumb_path = image_path`라 갤러리가 원본 PNG(장당 수 MB)를
//! 그대로 디코드했다. 여기서 최장변 512px JPEG(품질 80) 썸네일을 만들어
//! `thumb_path`를 분리한다.
//!
//! - 생성 직후: `spawn_generate` — gen://done을 막지 않는 백그라운드 후처리
//! - 기존 레코드: `backfill` — 앱 시작 후 오래된 순으로 스로틀 처리
//! - 실패해도 갤러리는 `thumbPath || imagePath` 폴백으로 원본을 보여주므로 치명적이지 않다
//!
//! 포맷 메모: docs/11은 WebP를 제안했지만 image crate 0.25는 무손실 WebP만
//! 지원한다(용량이 원본급). 새 네이티브 의존성 없이 가는 JPEG 80이 실리적 선택.

use std::path::{Path, PathBuf};

use crate::db::Db;
use crate::error::AppError;

pub const THUMB_MAX_SIDE: u32 = 512;
const JPEG_QUALITY: u8 = 80;

/// 원본 상대 경로 → 썸네일 상대 경로 (같은 폴더, 확장자만 `.thumb.jpg`).
pub fn thumb_rel_path(image_rel: &str) -> String {
    match image_rel.rsplit_once('.') {
        Some((stem, _ext)) => format!("{stem}.thumb.jpg"),
        None => format!("{image_rel}.thumb.jpg"),
    }
}

/// 원본을 최장변 512px 이하 JPEG로 축소 저장 (블로킹 — spawn_blocking에서 호출).
/// 원본이 이미 작으면 업스케일하지 않는다. JPEG는 알파 미지원이라 RGB로 변환.
pub fn generate_thumb_file(
    data_root: &Path,
    image_rel: &str,
    thumb_rel: &str,
) -> Result<(), AppError> {
    let src = data_root.join(image_rel);
    let img = image::open(&src)
        .map_err(|e| AppError::with_detail("E_THUMB_READ", "썸네일 원본을 읽지 못했어요.", e))?;
    let small = if img.width().max(img.height()) > THUMB_MAX_SIDE {
        img.thumbnail(THUMB_MAX_SIDE, THUMB_MAX_SIDE)
    } else {
        img
    };
    let rgb = small.to_rgb8();
    let dst = data_root.join(thumb_rel);
    let mut out =
        std::io::BufWriter::new(std::fs::File::create(&dst).map_err(|e| {
            AppError::with_detail("E_THUMB_WRITE", "썸네일을 저장하지 못했어요.", e)
        })?);
    rgb.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
        &mut out,
        JPEG_QUALITY,
    ))
    .map_err(|e| AppError::with_detail("E_THUMB_WRITE", "썸네일을 저장하지 못했어요.", e))?;
    Ok(())
}

/// 이미지 1장을 썸네일화하고 DB thumb_path를 갱신한다.
async fn process_one(db: &Db, data_root: &Path, id: &str, image_rel: &str) -> Result<(), AppError> {
    let thumb_rel = thumb_rel_path(image_rel);
    let root = data_root.to_path_buf();
    let image_rel2 = image_rel.to_string();
    let thumb_rel2 = thumb_rel.clone();
    tokio::task::spawn_blocking(move || generate_thumb_file(&root, &image_rel2, &thumb_rel2))
        .await
        .map_err(|e| AppError::with_detail("E_THUMB", "썸네일 작업이 중단됐어요.", e))??;
    db.set_thumb_path(id, &thumb_rel).await?;
    Ok(())
}

/// 생성 직후 백그라운드 썸네일 (T9.3, §P2.1). gen://done 지연 금지 —
/// 갤러리는 thumb_path가 아직 원본이어도 폴백으로 동작한다.
pub fn spawn_generate(db: Db, data_root: PathBuf, items: Vec<(String, String)>) {
    tokio::spawn(async move {
        for (id, image_rel) in items {
            if let Err(e) = process_one(&db, &data_root, &id, &image_rel).await {
                eprintln!("썸네일 생성 실패({id}): {e}");
            }
        }
    });
}

/// 기존 레코드 백필 (§P2.2): `thumb_path == image_path`인 행을 오래된 순으로
/// keyset 커서로 훑는다. `delay`는 장당 간격(스로틀) — 앱 시작 직후 IO를
/// 독점하지 않기 위함. 실패 행은 커서가 지나가므로 이번 실행에서 재시도하지 않는다.
pub async fn backfill(db: Db, data_root: PathBuf, delay: std::time::Duration) {
    let mut cursor: Option<(i64, String)> = None;
    let mut done = 0u64;
    loop {
        let rows = match db
            .list_thumbless(50, cursor.as_ref().map(|(t, id)| (*t, id.as_str())))
            .await
        {
            Ok(rows) => rows,
            Err(e) => {
                eprintln!("썸네일 백필 조회 실패: {e}");
                return;
            }
        };
        let Some(last) = rows.last() else { break };
        cursor = Some((last.0, last.1.clone()));
        for (created_at, id, image_rel) in &rows {
            let _ = created_at;
            match process_one(&db, &data_root, id, image_rel).await {
                Ok(()) => done += 1,
                Err(e) => eprintln!("썸네일 백필 실패({id}): {e}"),
            }
            tokio::time::sleep(delay).await;
        }
    }
    if done > 0 {
        eprintln!("썸네일 백필 완료: {done}장");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_rel_path_swaps_extension() {
        assert_eq!(
            thumb_rel_path("outputs/2026-07/abc.png"),
            "outputs/2026-07/abc.thumb.jpg"
        );
        assert_eq!(thumb_rel_path("noext"), "noext.thumb.jpg");
        // 루트 기준 상대 경로 유지 (CLAUDE.md 주의사항)
        assert!(!thumb_rel_path("outputs/2026-07/abc.png").starts_with('/'));
    }

    fn write_png(path: &Path, w: u32, h: u32) {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb([120, 40, 200]));
        img.save(path).unwrap();
    }

    #[test]
    fn thumb_is_downscaled_jpeg_under_limit() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("outputs/2026-07")).unwrap();
        write_png(&dir.path().join("outputs/2026-07/a.png"), 1024, 768);

        let rel = "outputs/2026-07/a.png";
        let thumb = thumb_rel_path(rel);
        generate_thumb_file(dir.path(), rel, &thumb).unwrap();

        let out = image::open(dir.path().join(&thumb)).unwrap();
        assert!(out.width() <= THUMB_MAX_SIDE && out.height() <= THUMB_MAX_SIDE);
        assert_eq!(out.width(), 512); // 최장변 기준 축소
                                      // 원본 대비 확실히 작아야 의미가 있다
        let orig = std::fs::metadata(dir.path().join(rel)).unwrap().len();
        let small = std::fs::metadata(dir.path().join(&thumb)).unwrap().len();
        assert!(small < orig, "thumb {small}B >= orig {orig}B");
    }

    #[test]
    fn small_image_is_not_upscaled() {
        let dir = tempfile::tempdir().unwrap();
        write_png(&dir.path().join("s.png"), 300, 200);
        generate_thumb_file(dir.path(), "s.png", "s.thumb.jpg").unwrap();
        let out = image::open(dir.path().join("s.thumb.jpg")).unwrap();
        assert_eq!((out.width(), out.height()), (300, 200));
    }

    #[tokio::test]
    async fn backfill_fills_thumbless_rows_and_skips_failures() {
        let db = Db::connect_in_memory().await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("outputs/2026-07")).unwrap();

        // 정상 2장 + 원본이 없는 1장 (실패해도 전체가 멈추면 안 됨)
        for (id, at) in [("g1", 100), ("g2", 200)] {
            write_png(
                &dir.path().join(format!("outputs/2026-07/{id}.png")),
                64,
                64,
            );
            db.insert_generation(&crate::db::models::Generation {
                id: id.into(),
                created_at: at,
                image_path: format!("outputs/2026-07/{id}.png"),
                thumb_path: format!("outputs/2026-07/{id}.png"), // 백필 대상 표식
                keyword_ko: None,
                prompt_final: "p".into(),
                negative: None,
                preset_id: None,
                preset_version: None,
                style_id: None,
                seed: 1,
                steps: None,
                cfg: None,
                width: Some(64),
                height: Some(64),
                model: None,
                favorite: false,
            })
            .await
            .unwrap();
        }
        let mut missing = crate::db::models::Generation {
            id: "broken".into(),
            created_at: 150,
            image_path: "outputs/2026-07/broken.png".into(),
            thumb_path: "outputs/2026-07/broken.png".into(),
            keyword_ko: None,
            prompt_final: "p".into(),
            negative: None,
            preset_id: None,
            preset_version: None,
            style_id: None,
            seed: 1,
            steps: None,
            cfg: None,
            width: None,
            height: None,
            model: None,
            favorite: false,
        };
        missing.thumb_path = missing.image_path.clone();
        db.insert_generation(&missing).await.unwrap();

        backfill(
            db.clone(),
            dir.path().to_path_buf(),
            std::time::Duration::ZERO,
        )
        .await;

        for id in ["g1", "g2"] {
            let g = db.get_generation(id).await.unwrap().unwrap();
            assert_eq!(g.thumb_path, format!("outputs/2026-07/{id}.thumb.jpg"));
            assert!(dir.path().join(&g.thumb_path).exists());
        }
        // 실패 행은 그대로 (다음 실행에서 재시도 기회)
        let broken = db.get_generation("broken").await.unwrap().unwrap();
        assert_eq!(broken.thumb_path, broken.image_path);
    }
}
