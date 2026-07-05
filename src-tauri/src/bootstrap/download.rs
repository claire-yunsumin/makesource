//! HTTP Range 이어받기 다운로더 (TAD §7).
//!
//! 내려받는 동안 `<dest>.part`에 쓰고, 완료 시 `<dest>`로 원자적 rename.
//! 재실행 시 `.part` 크기부터 Range 요청으로 이어받는다.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

use crate::error::AppError;

/// 진행 콜백: (받은 바이트, 전체 바이트(모르면 None))
pub type ProgressFn = Box<dyn Fn(u64, Option<u64>) + Send>;

/// 부분 파일 경로 규약.
pub fn part_path(dest: &Path) -> PathBuf {
    let mut os = dest.as_os_str().to_owned();
    os.push(".part");
    PathBuf::from(os)
}

/// 이어받기 시작 오프셋: `.part` 파일 크기 (없으면 0).
pub fn resume_offset(dest: &Path) -> u64 {
    std::fs::metadata(part_path(dest))
        .map(|m| m.len())
        .unwrap_or(0)
}

/// Range 헤더 값. 오프셋 0이면 None(전체 요청).
pub fn range_header(offset: u64) -> Option<String> {
    (offset > 0).then(|| format!("bytes={offset}-"))
}

/// `url`을 `dest`로 내려받는다. 이미 완료된 파일이 있으면 즉시 성공.
pub async fn download_resumable(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    on_progress: ProgressFn,
) -> Result<(), AppError> {
    if dest.exists() {
        return Ok(()); // 이미 완료 (재실행 멱등성)
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let part = part_path(dest);
    let offset = resume_offset(dest);

    let mut req = client.get(url);
    if let Some(range) = range_header(offset) {
        req = req.header(reqwest::header::RANGE, range);
    }
    let resp = req.send().await?.error_for_status()?;

    // 206이면 이어받기 인정, 200이면 서버가 Range 미지원 → 처음부터 다시
    let (mut written, append) =
        if offset > 0 && resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
            (offset, true)
        } else {
            (0, false)
        };
    let total = resp.content_length().map(|len| len + written);

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(append)
        .write(true)
        .truncate(!append)
        .open(&part)
        .await?;

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        written += chunk.len() as u64;
        on_progress(written, total);
    }
    file.flush().await?;
    drop(file);

    tokio::fs::rename(&part, dest).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_path_appends_suffix() {
        assert_eq!(
            part_path(Path::new("/m/sd_xl_base_1.0.safetensors")),
            Path::new("/m/sd_xl_base_1.0.safetensors.part")
        );
    }

    #[test]
    fn resume_offset_reads_part_size() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("model.bin");

        // .part 없음 → 0
        assert_eq!(resume_offset(&dest), 0);

        // .part 존재 → 크기부터 이어받기
        std::fs::write(part_path(&dest), b"12345").unwrap();
        assert_eq!(resume_offset(&dest), 5);
    }

    #[test]
    fn range_header_only_when_resuming() {
        assert_eq!(range_header(0), None);
        assert_eq!(range_header(1024), Some("bytes=1024-".to_string()));
    }

    #[tokio::test]
    async fn completed_file_short_circuits() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("done.bin");
        std::fs::write(&dest, b"complete").unwrap();

        // dest가 이미 있으면 네트워크 없이 성공해야 함 (무효 URL로 검증)
        let client = reqwest::Client::new();
        download_resumable(
            &client,
            "http://invalid.invalid/x",
            &dest,
            Box::new(|_, _| {}),
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"complete");
    }
}
