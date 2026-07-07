//! export_image (TAD §5, F-4.2).
//!
//! png는 원본 복사, jpg/webp는 image 크레이트로 변환(T3.2),
//! 투명 배경은 rembg(u2net) 서브프로세스로 배경 제거(T2.4b).

use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::{AppHandle, Manager, State};

use crate::db::Db;
use crate::error::AppError;
use crate::paths;

/// 원본은 레포 루트 `python/remove_bg.py` (TAD §2). translate.py와 같은 패턴으로
/// 바이너리에 내장했다가 실행 시점에 데이터 루트에 기록해 실행한다.
pub const REMOVE_BG_PY: &str = include_str!("../../../python/remove_bg.py");

/// u2net 추론은 CPU에서 수십 초까지 걸릴 수 있어 넉넉히.
const REMOVE_BG_TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArgs {
    pub id: String,
    /// png | jpg | webp
    pub format: String,
    pub transparent: Option<bool>,
    pub dest_dir: String,
}

/// 파일명에 쓸 수 있게 정리: 한글/영숫자/-/_만 유지, 공백→-, 최대 40자.
pub fn sanitize_filename_part(text: &str) -> String {
    let cleaned: String = text
        .trim()
        .chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '가'..='힣'))
        .collect();
    let truncated: String = cleaned.chars().take(40).collect();
    if truncated.is_empty() {
        "image".to_string()
    } else {
        truncated
    }
}

/// dest_dir 안에서 겹치지 않는 경로: `{base}.{ext}`, 겹치면 `{base}-2.{ext}`…
pub fn unique_path(dest_dir: &Path, base: &str, ext: &str) -> PathBuf {
    let first = dest_dir.join(format!("{base}.{ext}"));
    if !first.exists() {
        return first;
    }
    for n in 2..1000 {
        let candidate = dest_dir.join(format!("{base}-{n}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    // 사실상 도달 불가 — 마지막 후보 반환
    dest_dir.join(format!("{base}-999.{ext}"))
}

/// remove_bg.py stdout(JSON 한 줄) 파싱.
pub fn parse_remove_bg_output(line: &str) -> Result<(), String> {
    #[derive(Deserialize)]
    struct PyOut {
        ok: bool,
        error: Option<String>,
        detail: Option<String>,
    }
    let parsed: PyOut =
        serde_json::from_str(line.trim()).map_err(|e| format!("잘못된 출력: {e}"))?;
    if parsed.ok {
        Ok(())
    } else {
        Err(format!(
            "{}: {}",
            parsed.error.unwrap_or_else(|| "unknown".into()),
            parsed.detail.unwrap_or_default()
        ))
    }
}

/// rembg 서브프로세스로 배경 제거 PNG 저장 (T2.4b).
async fn run_remove_bg(
    python: &Path,
    script: &Path,
    src: &Path,
    dest: &Path,
    model_dir: &Path,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;

    let mut child = tokio::process::Command::new(python)
        .arg(script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("실행 실패: {e}"))?;

    let input = serde_json::json!({
        "src": src.to_string_lossy(),
        "dest": dest.to_string_lossy(),
        "modelDir": model_dir.to_string_lossy(),
    })
    .to_string()
        + "\n";
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|e| format!("stdin 쓰기 실패: {e}"))?;
    }
    drop(child.stdin.take());

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(REMOVE_BG_TIMEOUT_SECS),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| format!("{REMOVE_BG_TIMEOUT_SECS}초 안에 끝나지 않았어요"))?
    .map_err(|e| format!("프로세스 오류: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_remove_bg_output(stdout.lines().next().unwrap_or(""))
}

/// png는 그대로 복사, jpg/webp는 변환해서 저장한다 (블로킹 — spawn_blocking에서 호출).
fn write_converted(src: &Path, dest: &Path, format: &str) -> Result<(), AppError> {
    if format == "png" {
        std::fs::copy(src, dest).map_err(|e| {
            AppError::with_detail(
                "E_EXPORT_COPY",
                "이미지를 저장하지 못했어요.",
                format!("{} -> {}: {e}", src.display(), dest.display()),
            )
        })?;
        return Ok(());
    }
    let img = image::open(src)
        .map_err(|e| AppError::with_detail("E_IMAGE_DECODE", "원본 이미지를 읽지 못했어요.", e))?;
    let encode_err = |e: image::ImageError| {
        AppError::with_detail("E_IMAGE_ENCODE", "이미지를 변환하지 못했어요.", e)
    };
    match format {
        "jpg" => {
            // JPG는 알파 없음 — RGB로 변환 후 품질 90
            let file = std::fs::File::create(dest)?;
            let mut writer = std::io::BufWriter::new(file);
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 90);
            img.to_rgb8()
                .write_with_encoder(encoder)
                .map_err(encode_err)?;
        }
        "webp" => {
            // image 크레이트의 WebP 인코더는 무손실
            let file = std::fs::File::create(dest)?;
            let writer = std::io::BufWriter::new(file);
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(writer);
            img.to_rgba8()
                .write_with_encoder(encoder)
                .map_err(encode_err)?;
        }
        other => {
            return Err(AppError::with_detail(
                "E_FORMAT_UNSUPPORTED",
                "PNG, JPG, WebP로만 저장할 수 있어요.",
                other,
            ));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn export_image(
    app: AppHandle,
    db: State<'_, Db>,
    args: ExportArgs,
) -> Result<String, AppError> {
    if !matches!(args.format.as_str(), "png" | "jpg" | "webp") {
        return Err(AppError::with_detail(
            "E_FORMAT_UNSUPPORTED",
            "PNG, JPG, WebP로만 저장할 수 있어요.",
            args.format.clone(),
        ));
    }
    let transparent = args.transparent == Some(true);
    if transparent && args.format != "png" {
        return Err(AppError::new(
            "E_TRANSPARENT_FORMAT",
            "투명 배경은 PNG로만 저장할 수 있어요.",
        ));
    }

    let gen = db
        .get_generation(&args.id)
        .await
        .map_err(|e| AppError::with_detail("E_DB", "히스토리를 읽지 못했어요.", e))?
        .ok_or_else(|| AppError::new("E_GEN_NOT_FOUND", "이미지를 찾을 수 없어요."))?;

    let data_root = paths::app_data_root(
        &app.path()
            .data_dir()
            .map_err(|e| AppError::with_detail("E_PATH", "앱 데이터 폴더를 찾지 못했어요.", e))?,
    );
    let src = data_root.join(&gen.image_path);
    if !src.exists() {
        return Err(AppError::with_detail(
            "E_IMAGE_MISSING",
            "원본 이미지 파일이 없어요. 저장 위치를 옮겼다면 설정에서 확인해 주세요.",
            gen.image_path.clone(),
        ));
    }

    let dest_dir = PathBuf::from(&args.dest_dir);
    std::fs::create_dir_all(&dest_dir)?;
    // 파일명 규칙(기본값): {키워드}_{시드}.{포맷} — 규칙 설정 UI는 T7.1
    let base = format!(
        "{}_{}",
        sanitize_filename_part(gen.keyword_ko.as_deref().unwrap_or("image")),
        gen.seed
    );
    let dest = unique_path(&dest_dir, &base, &args.format);

    if transparent {
        // 배경 제거 경로 (T2.4b): venv 파이썬 + rembg
        let python = data_root.join("runtime/venv/bin/python");
        if !python.exists() {
            return Err(AppError::new(
                "E_BG_TOOL_NOT_READY",
                "배경 제거 도구가 아직 설치되지 않았어요. 처음 사용 설정(엔진 설치)을 마치면 쓸 수 있어요.",
            ));
        }
        let script_dir = data_root.join("runtime");
        std::fs::create_dir_all(&script_dir)?;
        let script = script_dir.join("remove_bg.py");
        paths::write_if_changed(&script, REMOVE_BG_PY)?;
        run_remove_bg(
            &python,
            &script,
            &src,
            &dest,
            &data_root.join("models/rembg"),
        )
        .await
        .map_err(|detail| {
            AppError::with_detail("E_BG_REMOVE", "배경을 제거하지 못했어요.", detail)
        })?;
        return Ok(dest.to_string_lossy().into_owned());
    }

    // 이미지 디코드/인코드는 CPU 작업 — command 스레드를 막지 않는다
    let format = args.format.clone();
    let dest2 = dest.clone();
    tauri::async_runtime::spawn_blocking(move || write_converted(&src, &dest2, &format))
        .await
        .map_err(|e| AppError::with_detail("E_EXPORT_TASK", "이미지를 저장하지 못했어요.", e))??;
    Ok(dest.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_keeps_korean_and_alnum_and_replaces_spaces() {
        assert_eq!(sanitize_filename_part("통나무집"), "통나무집");
        assert_eq!(sanitize_filename_part("log cabin 2"), "log-cabin-2");
        assert_eq!(sanitize_filename_part("  a/b\\c:d*e  "), "abcde");
        assert_eq!(sanitize_filename_part("!!!"), "image");
        assert_eq!(sanitize_filename_part(""), "image");
    }

    #[test]
    fn sanitize_truncates_to_40_chars() {
        let long = "가".repeat(80);
        assert_eq!(sanitize_filename_part(&long).chars().count(), 40);
    }

    #[test]
    fn write_converted_produces_decodable_jpg_and_webp() {
        let dir = tempfile::tempdir().unwrap();
        // 4x4 원본 PNG 생성
        let src = dir.path().join("src.png");
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 100, 50, 255]));
        img.save(&src).unwrap();

        for format in ["png", "jpg", "webp"] {
            let dest = dir.path().join(format!("out.{format}"));
            write_converted(&src, &dest, format).unwrap();
            let decoded = image::open(&dest).unwrap();
            assert_eq!(decoded.width(), 4, "{format} 왕복 실패");
        }

        let err = write_converted(&src, &dir.path().join("out.gif"), "gif").unwrap_err();
        assert_eq!(err.code, "E_FORMAT_UNSUPPORTED");
    }

    #[test]
    fn remove_bg_output_parsing() {
        assert!(parse_remove_bg_output(r#"{"ok":true,"dest":"/x.png"}"#).is_ok());
        let err = parse_remove_bg_output(r#"{"ok":false,"error":"model_missing","detail":"d"}"#)
            .unwrap_err();
        assert!(err.contains("model_missing"));
        assert!(parse_remove_bg_output("garbage").is_err());
    }

    #[tokio::test]
    async fn fake_python_remove_bg_roundtrip() {
        // 계약(stdin JSON → stdout JSON 한 줄)을 흉내내는 가짜 파이썬 — dest 파일 생성
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("python");
        std::fs::write(
            &fake,
            "#!/bin/sh\nread line\ndest=$(echo \"$line\" | sed 's/.*\"dest\": *\"\\([^\"]*\\)\".*/\\1/')\ntouch \"$dest\"\necho \"{\\\"ok\\\":true,\\\"dest\\\":\\\"$dest\\\"}\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let src = dir.path().join("src.png");
        std::fs::write(&src, b"x").unwrap();
        let dest = dir.path().join("out.png");
        run_remove_bg(&fake, Path::new("/dev/null"), &src, &dest, dir.path())
            .await
            .unwrap();
        assert!(dest.exists());
    }

    #[test]
    fn unique_path_appends_counter_on_collision() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            unique_path(dir.path(), "통나무집_42", "png"),
            dir.path().join("통나무집_42.png")
        );
        std::fs::write(dir.path().join("통나무집_42.png"), b"x").unwrap();
        assert_eq!(
            unique_path(dir.path(), "통나무집_42", "png"),
            dir.path().join("통나무집_42-2.png")
        );
        std::fs::write(dir.path().join("통나무집_42-2.png"), b"x").unwrap();
        assert_eq!(
            unique_path(dir.path(), "통나무집_42", "png"),
            dir.path().join("통나무집_42-3.png")
        );
    }
}
