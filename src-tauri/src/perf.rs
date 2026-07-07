//! 로컬 성능 계측 (T9.1, docs/11 §P0).
//!
//! 생성 파이프라인의 단계별 소요를 `logs/perf.log`에 JSON 한 줄로 남긴다.
//! 절대 규칙 1: 계측은 전부 로컬 파일 — 외부 전송 없음. 프롬프트·키워드 등
//! 내용은 기록하지 않고 소요 시간·크기 파라미터만 남긴다.

use std::path::Path;
use std::time::Instant;

use serde_json::{json, Value};

/// 단계별 스톱워치. `mark()` 호출 사이 구간을 순서대로 기록한다.
pub struct StageTimer {
    started: Instant,
    last: Instant,
    stages: Vec<(&'static str, u128)>,
}

impl Default for StageTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl StageTimer {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            started: now,
            last: now,
            stages: Vec::new(),
        }
    }

    /// 직전 mark(또는 시작) 이후 구간을 `name`으로 기록.
    pub fn mark(&mut self, name: &'static str) {
        let now = Instant::now();
        self.stages
            .push((name, now.duration_since(self.last).as_millis()));
        self.last = now;
    }

    pub fn total_ms(&self) -> u128 {
        self.started.elapsed().as_millis()
    }

    /// perf.log 한 줄 페이로드. `extra`의 키는 최상위에 병합된다.
    pub fn to_log_value(&self, kind: &str, extra: Value) -> Value {
        let mut obj = json!({
            "ts": chrono::Utc::now().timestamp_millis(),
            "kind": kind,
            "totalMs": self.total_ms() as u64,
            "stages": Value::Object(
                self.stages
                    .iter()
                    .map(|(name, ms)| ((*name).to_string(), json!(*ms as u64)))
                    .collect(),
            ),
        });
        if let (Some(base), Some(extra)) = (obj.as_object_mut(), extra.as_object()) {
            for (k, v) in extra {
                base.insert(k.clone(), v.clone());
            }
        }
        obj
    }
}

/// `logs/perf.log`에 JSON 한 줄 추가. 계측 실패가 본 작업을 막으면 안 되므로
/// 에러는 stderr로만 남기고 삼킨다.
pub fn append_perf_line(data_root: &Path, line: &Value) {
    use std::io::Write;

    let dir = data_root.join("logs");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("perf.log 폴더 생성 실패: {e}");
        return;
    }
    let result = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("perf.log"))
        .and_then(|mut f| writeln!(f, "{line}"));
    if let Err(e) = result {
        eprintln!("perf.log 기록 실패: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_timer_records_all_stages_with_extras() {
        let mut t = StageTimer::new();
        t.mark("prepare");
        t.mark("engine");
        t.mark("persist");
        let v = t.to_log_value("generate", json!({"jobId": "j1", "batch": 2}));

        assert_eq!(v["kind"], "generate");
        assert_eq!(v["jobId"], "j1");
        assert_eq!(v["batch"], 2);
        assert!(v["ts"].as_i64().unwrap() > 0);
        // serde_json Map은 키 정렬 — 순서가 아니라 존재·값으로 검증
        let stages = v["stages"].as_object().unwrap();
        for key in ["prepare", "engine", "persist"] {
            assert!(stages.contains_key(key), "{key} 누락: {stages:?}");
        }
        // 합산 오차: 단계 합 ≤ 총합 (P0.1 AC의 성립 조건)
        let sum: u64 = stages.values().map(|v| v.as_u64().unwrap()).sum();
        assert!(sum <= v["totalMs"].as_u64().unwrap() + 1);
    }

    #[test]
    fn append_creates_log_file_with_one_json_line_per_call() {
        let dir = tempfile::tempdir().unwrap();
        append_perf_line(dir.path(), &json!({"kind": "generate", "totalMs": 1}));
        append_perf_line(dir.path(), &json!({"kind": "generate", "totalMs": 2}));

        let text = std::fs::read_to_string(dir.path().join("logs/perf.log")).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        for line in lines {
            let parsed: Value = serde_json::from_str(line).unwrap();
            assert_eq!(parsed["kind"], "generate");
        }
    }
}
