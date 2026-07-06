//! kohya sd-scripts stdout/stderr 파서 (TAD §8 4, T6.3).
//!
//! tqdm 진행줄에서 step/total/loss/ETA를, 로그줄에서 epoch 경계와 체크포인트
//! 저장 경로를 뽑는다. 외부 regex 의존 없이 수동 파싱 — kohya 출력 형식이
//! 바뀌어도 조용히 None을 돌려주는(크래시 없는) 쪽으로 설계.

/// 파싱된 학습 이벤트 한 건.
#[derive(Debug, Clone, PartialEq)]
pub enum TrainEvent {
    /// tqdm 진행줄: `steps:  30%|███| 300/1000 [00:12<00:28, 24.3it/s, avg_loss=0.0512]`
    Progress {
        step: u32,
        total: u32,
        loss: Option<f64>,
        eta_seconds: Option<u64>,
    },
    /// `epoch 2/4` 형태의 epoch 경계
    Epoch { current: u32, total: u32 },
    /// 체크포인트/샘플 저장: `saving checkpoint: /path/x.safetensors`
    Saved { path: String },
}

/// `MM:SS` 또는 `HH:MM:SS` → 초.
fn parse_clock(text: &str) -> Option<u64> {
    let parts: Vec<&str> = text.split(':').collect();
    let nums: Vec<u64> = parts
        .iter()
        .map(|p| p.trim().parse::<u64>())
        .collect::<Result<_, _>>()
        .ok()?;
    match nums.as_slice() {
        [m, s] => Some(m * 60 + s),
        [h, m, s] => Some(h * 3600 + m * 60 + s),
        _ => None,
    }
}

/// `key=값` 조각에서 f64 값 추출 (`avg_loss=0.0512` 등, 뒤에 `]`가 붙을 수 있음).
fn parse_keyed_f64(line: &str, key: &str) -> Option<f64> {
    let start = line.find(key)? + key.len();
    let rest = &line[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == '+'))
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// tqdm 진행줄에서 `current/total [elapsed<remaining, ...]` 파싱.
fn parse_progress(line: &str) -> Option<TrainEvent> {
    // "300/1000 [" 패턴 찾기 — 브래킷 직전의 slash 쌍이 스텝 카운터
    let bracket = line.find('[')?;
    let before = line[..bracket].trim_end();
    let slash_field = before
        .rsplit(|c: char| c.is_whitespace() || c == '|')
        .next()?;
    let (cur, total) = slash_field.split_once('/')?;
    let step: u32 = cur.trim().parse().ok()?;
    let total: u32 = total.trim().parse().ok()?;
    if total == 0 {
        return None;
    }

    // "[00:12<00:28, ..." → remaining이 ETA. '?'(추정 불가)는 None
    let inside = &line[bracket + 1..];
    let eta_seconds = inside
        .split_once('<')
        .and_then(|(_, rest)| rest.split([',', ']']).next())
        .and_then(|t| parse_clock(t.trim()));

    let loss = parse_keyed_f64(line, "avg_loss=").or_else(|| parse_keyed_f64(line, "loss="));

    Some(TrainEvent::Progress {
        step,
        total,
        loss,
        eta_seconds,
    })
}

/// 한 줄 파싱. 관심 없는 줄은 None (에러 아님 — 로그는 그대로 파일로).
pub fn parse_line(line: &str) -> Option<TrainEvent> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // epoch 경계: "epoch 2/4" (kohya 로그, 대소문자 무시)
    let lower = line.to_lowercase();
    if let Some(rest) = lower.strip_prefix("epoch ") {
        if let Some((cur, total)) = rest
            .split_whitespace()
            .next()
            .and_then(|f| f.split_once('/'))
        {
            if let (Ok(current), Ok(total)) = (cur.parse(), total.parse()) {
                return Some(TrainEvent::Epoch { current, total });
            }
        }
    }

    // 체크포인트/모델 저장 경로
    for prefix in ["saving checkpoint:", "model saved.", "saving model:"] {
        if let Some(idx) = lower.find(prefix) {
            let path = line[idx + prefix.len()..].trim();
            if !path.is_empty() {
                return Some(TrainEvent::Saved {
                    path: path.to_string(),
                });
            }
        }
    }

    // tqdm 진행줄 ("steps:" 접두 또는 N/M [ 패턴)
    if lower.contains("it/s") || lower.starts_with("steps") {
        return parse_progress(line);
    }
    None
}

/// tqdm은 캐리지 리턴(\r)으로 같은 줄을 덮어쓴다 — 청크를 논리 줄로 분해.
pub fn split_carriage_lines(chunk: &str) -> Vec<&str> {
    chunk
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tqdm_progress_with_loss_and_eta() {
        let line = "steps:  30%|███       | 300/1000 [00:12<00:28, 24.31it/s, avg_loss=0.0512]";
        assert_eq!(
            parse_line(line),
            Some(TrainEvent::Progress {
                step: 300,
                total: 1000,
                loss: Some(0.0512),
                eta_seconds: Some(28),
            })
        );
    }

    #[test]
    fn parses_progress_with_hour_scale_eta_and_plain_loss_key() {
        let line = "steps:  5%|▌| 60/1200 [03:00<1:02:30, 3.1it/s, loss=0.213]";
        assert_eq!(
            parse_line(line),
            Some(TrainEvent::Progress {
                step: 60,
                total: 1200,
                loss: Some(0.213),
                eta_seconds: Some(3750),
            })
        );
    }

    #[test]
    fn progress_without_postfix_fields_still_parses() {
        // 시작 직후 tqdm은 rate·loss가 없을 수 있고 ETA가 '?'일 수 있다
        let line = "steps:   0%|          | 0/1000 [00:00<?, ?it/s]";
        assert_eq!(
            parse_line(line),
            Some(TrainEvent::Progress {
                step: 0,
                total: 1000,
                loss: None,
                eta_seconds: None,
            })
        );
    }

    #[test]
    fn parses_epoch_boundary() {
        assert_eq!(
            parse_line("epoch 2/4"),
            Some(TrainEvent::Epoch {
                current: 2,
                total: 4
            })
        );
        assert_eq!(
            parse_line("Epoch 10/12 is starting"),
            Some(TrainEvent::Epoch {
                current: 10,
                total: 12
            })
        );
    }

    #[test]
    fn parses_checkpoint_save_path() {
        assert_eq!(
            parse_line("saving checkpoint: /data/training/j1/output/style-000002.safetensors"),
            Some(TrainEvent::Saved {
                path: "/data/training/j1/output/style-000002.safetensors".to_string()
            })
        );
    }

    #[test]
    fn irrelevant_and_malformed_lines_are_none_not_errors() {
        for line in [
            "",
            "prepare optimizer, data loader etc.",
            "use xformers for U-Net",
            "steps: garbage without numbers [",
            "steps: 12/0 [00:01<00:02]", // total 0 — 나누기 방지
        ] {
            assert_eq!(parse_line(line), None, "{line:?}");
        }
    }

    #[test]
    fn splits_carriage_return_overwrites_into_logical_lines() {
        let chunk = "steps: 1/10 [00:01<00:09, 1it/s]\rsteps: 2/10 [00:02<00:08, 1it/s]\nepoch 1/2";
        let lines = split_carriage_lines(chunk);
        assert_eq!(lines.len(), 3);
        assert!(lines[1].contains("2/10"));
    }
}
