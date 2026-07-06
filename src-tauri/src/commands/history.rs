//! history_list / history_toggle_favorite (TAD §5).

use serde::Deserialize;
use tauri::State;

use crate::db::models::Generation;
use crate::db::{Db, HistoryFilter};
use crate::error::AppError;

/// 기본·최대 페이지 크기 (무한 스크롤 한 페이지).
const PAGE_SIZE: i64 = 40;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryListArgs {
    /// 키워드(한글)·프롬프트(영문) 부분 일치 검색 (T3.3)
    pub query: Option<String>,
    pub style_id: Option<String>,
    /// true = ♥만
    pub favorite: Option<bool>,
    /// 직전 페이지 마지막 항목의 커서 (`"{createdAt}:{id}"`). 없으면 첫 페이지.
    pub cursor: Option<String>,
}

/// 커서 문자열 파싱: `"{createdAt}:{id}"`. 형식이 틀리면 None (첫 페이지 취급 대신 에러로).
pub fn parse_cursor(cursor: &str) -> Option<(i64, &str)> {
    let (at, id) = cursor.split_once(':')?;
    let at: i64 = at.parse().ok()?;
    if id.is_empty() {
        return None;
    }
    Some((at, id))
}

#[tauri::command]
pub async fn history_list(
    db: State<'_, Db>,
    args: Option<HistoryListArgs>,
) -> Result<Vec<Generation>, AppError> {
    let args = args.unwrap_or_default();
    let cursor = match args.cursor.as_deref() {
        Some(raw) => Some(parse_cursor(raw).ok_or_else(|| {
            AppError::with_detail("E_CURSOR", "목록을 이어서 불러오지 못했어요.", raw)
        })?),
        None => None,
    };
    let filter = HistoryFilter {
        query: args.query,
        style_id: args.style_id,
        favorite: args.favorite,
    };
    db.list_generations_page(PAGE_SIZE, cursor, &filter)
        .await
        .map_err(|e| AppError::with_detail("E_DB", "히스토리를 읽지 못했어요.", e))
}

#[tauri::command]
pub async fn history_toggle_favorite(db: State<'_, Db>, id: String) -> Result<(), AppError> {
    let gen = db
        .get_generation(&id)
        .await
        .map_err(|e| AppError::with_detail("E_DB", "히스토리를 읽지 못했어요.", e))?
        .ok_or_else(|| AppError::new("E_GEN_NOT_FOUND", "이미지를 찾을 수 없어요."))?;
    db.set_favorite(&id, !gen.favorite)
        .await
        .map_err(|e| AppError::with_detail("E_DB", "즐겨찾기를 저장하지 못했어요.", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_cursor;

    #[test]
    fn cursor_parses_created_at_and_id() {
        assert_eq!(
            parse_cursor("1700000000000:abc-def"),
            Some((1_700_000_000_000, "abc-def"))
        );
        // 음수 created_at도 형식상 허용
        assert_eq!(parse_cursor("-5:x"), Some((-5, "x")));
    }

    #[test]
    fn invalid_cursor_is_rejected() {
        assert_eq!(parse_cursor(""), None);
        assert_eq!(parse_cursor("no-colon"), None);
        assert_eq!(parse_cursor("abc:id"), None);
        assert_eq!(parse_cursor("123:"), None);
    }
}
