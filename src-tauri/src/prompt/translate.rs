//! 한→영 키워드 변환 (TAD §4 ①②③).
//!
//! ① 도메인 용어 사전 — 내장 `resources/dict.ko-en.json` + 데이터 루트
//!    `dict.ko-en.json` 오버라이드(사용자 항목 우선)
//! ② Argos Translate — `python/translate.py`를 venv 파이썬 서브프로세스로 실행
//! ③ 실패 시 원문 그대로 + 사용자 경고 (04 §6 톤)
//!
//! 한글이 없는 키워드는 변환하지 않는다 (notNeeded).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_DICT: &str = include_str!("../../resources/dict.ko-en.json");

/// 원본은 레포 루트 `python/translate.py` (TAD §2). 바이너리에 내장했다가
/// 실행 시점에 데이터 루트로 기록해 dev/CLI/번들 어디서든 같은 경로로 실행한다.
pub const TRANSLATE_PY: &str = include_str!("../../../python/translate.py");

/// Argos 첫 호출은 모델 로드로 수 초가 걸릴 수 있어 넉넉히 잡는다.
const ARGOS_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TranslationSource {
    /// 한글이 없어 변환 불필요
    NotNeeded,
    /// 용어 사전 (§4 ①)
    Dict,
    /// Argos Translate (§4 ②)
    Argos,
    /// 변환 실패 — 원문 그대로 (§4 ③)
    Passthrough,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Translation {
    pub translated: String,
    pub source: TranslationSource,
    /// passthrough일 때 사용자 고지 문구 (04 §6 톤)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DictFile {
    #[allow(dead_code)]
    schema_version: u32,
    entries: HashMap<String, String>,
}

/// 한글(완성형 음절 + 자모) 포함 여부.
pub fn contains_hangul(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c,
            '\u{AC00}'..='\u{D7A3}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}'
        )
    })
}

/// 사전 로드: 내장 기본 + 데이터 루트 `dict.ko-en.json` 병합(사용자 항목 우선).
/// 사용자 파일이 깨져 있으면 무시하고 기본만 사용한다 (생성 흐름을 막지 않기 위해).
pub fn load_dict(data_root: &Path) -> HashMap<String, String> {
    let mut entries = serde_json::from_str::<DictFile>(DEFAULT_DICT)
        .map(|f| f.entries)
        .unwrap_or_default();
    let user_path = data_root.join("dict.ko-en.json");
    if let Ok(text) = std::fs::read_to_string(&user_path) {
        match serde_json::from_str::<DictFile>(&text) {
            Ok(user) => entries.extend(user.entries),
            Err(e) => eprintln!("사용자 사전 파싱 실패({}): {e}", user_path.display()),
        }
    }
    entries
}

/// 사전 변환 (§4 ①): 전체 문구 일치 우선, 아니면 공백 토큰 단위.
/// 한글 토큰이 사전에 없으면 실패(None) — Argos로 넘긴다.
pub fn dict_translate(keyword: &str, dict: &HashMap<String, String>) -> Option<String> {
    let trimmed = keyword.trim();
    if let Some(v) = dict.get(trimmed) {
        return Some(v.clone());
    }
    let mut out: Vec<&str> = Vec::new();
    for token in trimmed.split_whitespace() {
        match dict.get(token) {
            Some(v) => out.push(v),
            None if contains_hangul(token) => return None,
            None => out.push(token),
        }
    }
    Some(out.join(" "))
}

/// translate.py stdout(JSON 한 줄) 파싱. Ok(번역) 또는 Err(에러 요약).
pub fn parse_py_output(line: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct PyOut {
        ok: bool,
        translated: Option<String>,
        error: Option<String>,
        detail: Option<String>,
    }
    let parsed: PyOut =
        serde_json::from_str(line.trim()).map_err(|e| format!("잘못된 출력: {e}"))?;
    if parsed.ok {
        match parsed.translated {
            Some(t) if !t.trim().is_empty() => Ok(t),
            _ => Err("ok인데 translated가 비어 있음".into()),
        }
    } else {
        Err(format!(
            "{}: {}",
            parsed.error.unwrap_or_else(|| "unknown".into()),
            parsed.detail.unwrap_or_default()
        ))
    }
}

/// venv 파이썬 경로 (TAD §3 — engine::EngineConfig와 같은 규약).
fn venv_python(data_root: &Path) -> PathBuf {
    data_root.join("runtime/venv/bin/python")
}

/// 내장 translate.py를 데이터 루트에 기록하고 경로를 돌려준다.
fn ensure_script(data_root: &Path) -> std::io::Result<PathBuf> {
    let dir = data_root.join("runtime");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("translate.py");
    std::fs::write(&path, TRANSLATE_PY)?;
    Ok(path)
}

/// Argos 서브프로세스 실행 (§4 ②).
async fn run_argos(python: &Path, script: &Path, text: &str) -> Result<String, String> {
    use tokio::io::AsyncWriteExt;

    let mut child = tokio::process::Command::new(python)
        .arg(script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("실행 실패: {e}"))?;

    let input = serde_json::json!({ "text": text }).to_string() + "\n";
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|e| format!("stdin 쓰기 실패: {e}"))?;
    }
    drop(child.stdin.take());

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(ARGOS_TIMEOUT_SECS),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| format!("{ARGOS_TIMEOUT_SECS}초 안에 응답이 없어요"))?
    .map_err(|e| format!("프로세스 오류: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().unwrap_or("");
    parse_py_output(line)
}

/// 한→영 변환 전체 흐름 (§4 ①②③). 실패해도 Err을 내지 않고 원문 폴백을 돌려준다.
pub async fn translate_keyword(data_root: &Path, keyword: &str) -> Translation {
    let trimmed = keyword.trim();
    if !contains_hangul(trimmed) {
        return Translation {
            translated: trimmed.to_string(),
            source: TranslationSource::NotNeeded,
            warning: None,
        };
    }

    // ① 용어 사전
    let dict = load_dict(data_root);
    if let Some(translated) = dict_translate(trimmed, &dict) {
        return Translation {
            translated,
            source: TranslationSource::Dict,
            warning: None,
        };
    }

    // ② Argos (venv가 준비된 경우만)
    let python = venv_python(data_root);
    if python.exists() {
        match ensure_script(data_root) {
            Ok(script) => match run_argos(&python, &script, trimmed).await {
                Ok(translated) => {
                    return Translation {
                        translated,
                        source: TranslationSource::Argos,
                        warning: None,
                    };
                }
                Err(detail) => eprintln!("Argos 변환 실패: {detail}"),
            },
            Err(e) => eprintln!("translate.py 기록 실패: {e}"),
        }
    }

    // ③ 원문 폴백 + 경고 (04 §6 톤)
    Translation {
        translated: trimmed.to_string(),
        source: TranslationSource::Passthrough,
        warning: Some(
            "키워드를 영어로 바꾸지 못해 입력한 그대로 사용해요. 영문 키워드를 쓰면 결과가 더 정확해져요.".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dict_parses_and_has_log_cabin() {
        let dict: DictFile = serde_json::from_str(DEFAULT_DICT).unwrap();
        assert_eq!(dict.schema_version, 1);
        // M2 통합 AC 키워드 (docs/05): "통나무집"
        assert_eq!(dict.entries.get("통나무집").unwrap(), "log cabin");
    }

    #[test]
    fn hangul_detection() {
        assert!(contains_hangul("통나무집"));
        assert!(contains_hangul("red 자동차"));
        assert!(contains_hangul("ㅋㅋ")); // 자모
        assert!(!contains_hangul("log cabin"));
        assert!(!contains_hangul("123 !@#"));
        assert!(!contains_hangul(""));
    }

    #[test]
    fn dict_exact_match_wins_over_tokens() {
        let mut dict = HashMap::new();
        dict.insert("커피잔".to_string(), "coffee cup".to_string());
        dict.insert("커피".to_string(), "coffee".to_string());
        assert_eq!(dict_translate("커피잔", &dict).unwrap(), "coffee cup");
        assert_eq!(dict_translate("  커피잔  ", &dict).unwrap(), "coffee cup");
    }

    #[test]
    fn dict_translates_token_wise_and_keeps_english() {
        let mut dict = HashMap::new();
        dict.insert("빨간".to_string(), "red".to_string());
        dict.insert("자동차".to_string(), "car".to_string());
        assert_eq!(dict_translate("빨간 자동차", &dict).unwrap(), "red car");
        assert_eq!(dict_translate("cute 자동차", &dict).unwrap(), "cute car");
    }

    #[test]
    fn dict_fails_when_hangul_token_missing() {
        let mut dict = HashMap::new();
        dict.insert("자동차".to_string(), "car".to_string());
        assert_eq!(dict_translate("멋진 자동차", &dict), None);
    }

    #[test]
    fn user_dict_overrides_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("dict.ko-en.json"),
            r#"{"schemaVersion":1,"entries":{"통나무집":"wooden cabin","우리브랜드":"ourbrand"}}"#,
        )
        .unwrap();
        let dict = load_dict(dir.path());
        assert_eq!(dict.get("통나무집").unwrap(), "wooden cabin"); // 사용자 우선
        assert_eq!(dict.get("우리브랜드").unwrap(), "ourbrand"); // 사용자 추가
        assert_eq!(dict.get("로봇").unwrap(), "robot"); // 기본 유지
    }

    #[test]
    fn broken_user_dict_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("dict.ko-en.json"), "not json").unwrap();
        let dict = load_dict(dir.path());
        assert_eq!(dict.get("로봇").unwrap(), "robot");
    }

    #[test]
    fn py_output_parsing() {
        assert_eq!(
            parse_py_output(r#"{"ok":true,"translated":"log cabin","engine":"argos"}"#).unwrap(),
            "log cabin"
        );
        let err = parse_py_output(r#"{"ok":false,"error":"argos_unavailable","detail":"x"}"#)
            .unwrap_err();
        assert!(err.contains("argos_unavailable"));
        assert!(parse_py_output("garbage").is_err());
        assert!(parse_py_output(r#"{"ok":true,"translated":""}"#).is_err());
    }

    #[tokio::test]
    async fn english_keyword_needs_no_translation() {
        let dir = tempfile::tempdir().unwrap();
        let t = translate_keyword(dir.path(), "log cabin").await;
        assert_eq!(t.source, TranslationSource::NotNeeded);
        assert_eq!(t.translated, "log cabin");
        assert!(t.warning.is_none());
    }

    #[tokio::test]
    async fn dict_hit_translates_without_python() {
        let dir = tempfile::tempdir().unwrap();
        let t = translate_keyword(dir.path(), "통나무집").await;
        assert_eq!(t.source, TranslationSource::Dict);
        assert_eq!(t.translated, "log cabin");
    }

    #[tokio::test]
    async fn missing_venv_falls_through_to_passthrough_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        // 사전에 없는 한글 + venv 없음 → 원문 + 경고 (§4 ③)
        let t = translate_keyword(dir.path(), "각청뿔단검").await;
        assert_eq!(t.source, TranslationSource::Passthrough);
        assert_eq!(t.translated, "각청뿔단검");
        assert!(t.warning.is_some());
    }

    #[tokio::test]
    async fn fake_python_argos_roundtrip() {
        // 실제 Argos 대신 계약(stdin JSON → stdout JSON 한 줄)을 흉내내는 가짜 파이썬
        let dir = tempfile::tempdir().unwrap();
        let venv_bin = dir.path().join("runtime/venv/bin");
        std::fs::create_dir_all(&venv_bin).unwrap();
        let fake = venv_bin.join("python");
        std::fs::write(
            &fake,
            "#!/bin/sh\nread _line\necho '{\"ok\":true,\"translated\":\"fake result\",\"engine\":\"argos\"}'\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let t = translate_keyword(dir.path(), "사전에없는한글").await;
        assert_eq!(t.source, TranslationSource::Argos);
        assert_eq!(t.translated, "fake result");
        // 내장 스크립트가 데이터 루트에 기록되었는지 (레포 python/translate.py와 동일)
        let written = std::fs::read_to_string(dir.path().join("runtime/translate.py")).unwrap();
        assert_eq!(written, TRANSLATE_PY);
    }
}
