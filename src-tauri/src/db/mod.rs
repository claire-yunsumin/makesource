//! SQLite 접근 계층 (TAD §2 `db/`, §3.1 스키마).
//!
//! 마이그레이션은 `migrations/`의 SQL 파일을 sqlx Migrator로 임베드해 실행한다.
//! CRUD는 여기서 `Result<_, sqlx::Error>`로 노출하고, Tauri command 계층에서
//! AppError(TAD §5/§9)로 변환한다.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

pub mod models;

use models::{Generation, TrainingJob};

/// `migrations/`의 SQL을 컴파일 타임에 임베드한 마이그레이터.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// SQLite 커넥션 풀 래퍼. `tauri::App`의 managed state로 보관한다.
#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

/// history_list 검색·필터 조건 (T3.3, TAD §5).
#[derive(Debug, Default, Clone)]
pub struct HistoryFilter {
    /// keyword_ko 또는 prompt_final 부분 일치
    pub query: Option<String>,
    pub style_id: Option<String>,
    /// Some(true) = ♥만
    pub favorite: Option<bool>,
}

/// LIKE 패턴 이스케이프 (%, _, \ — ESCAPE '\\'와 세트).
fn like_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

impl Db {
    /// 파일 기반 DB에 연결하고 마이그레이션을 적용한다.
    /// 부모 디렉터리가 없으면 생성하고, DB 파일이 없으면 만든다.
    pub async fn connect(db_path: &Path) -> Result<Self, sqlx::Error> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(sqlx::Error::Io)?;
        }
        let opts = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new().connect_with(opts).await?;
        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    /// 테스트용 인메모리 DB. 인메모리는 커넥션마다 별도 DB이므로 풀을 1로 고정한다.
    #[cfg(test)]
    pub async fn connect_in_memory() -> Result<Self, sqlx::Error> {
        use std::str::FromStr;
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await?;
        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    // --- generations ---

    pub async fn insert_generation(&self, g: &Generation) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO generations (
                id, created_at, image_path, thumb_path, keyword_ko, prompt_final, negative,
                preset_id, preset_version, style_id, seed, steps, cfg, width, height, model, favorite
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&g.id)
        .bind(g.created_at)
        .bind(&g.image_path)
        .bind(&g.thumb_path)
        .bind(&g.keyword_ko)
        .bind(&g.prompt_final)
        .bind(&g.negative)
        .bind(&g.preset_id)
        .bind(g.preset_version)
        .bind(&g.style_id)
        .bind(g.seed)
        .bind(g.steps)
        .bind(g.cfg)
        .bind(g.width)
        .bind(g.height)
        .bind(&g.model)
        .bind(g.favorite)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_generation(&self, id: &str) -> Result<Option<Generation>, sqlx::Error> {
        sqlx::query_as::<_, Generation>("SELECT * FROM generations WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// 최신순 목록 (커서 기반 페이징은 history_list command에서 확장).
    pub async fn list_generations(&self, limit: i64) -> Result<Vec<Generation>, sqlx::Error> {
        sqlx::query_as::<_, Generation>(
            "SELECT * FROM generations ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// 최신순 keyset 페이징 + 검색·필터 (T3.1/T3.3, TAD §5 history_list).
    /// cursor는 직전 페이지 마지막 행의 (created_at, id) — 그보다 오래된 행부터 반환한다.
    /// created_at이 같은 행은 id DESC로 안정 정렬.
    pub async fn list_generations_page(
        &self,
        limit: i64,
        cursor: Option<(i64, &str)>,
        filter: &HistoryFilter,
    ) -> Result<Vec<Generation>, sqlx::Error> {
        let mut qb = sqlx::QueryBuilder::new("SELECT * FROM generations WHERE 1=1");
        if let Some((created_at, id)) = cursor {
            qb.push(" AND (created_at < ")
                .push_bind(created_at)
                .push(" OR (created_at = ")
                .push_bind(created_at)
                .push(" AND id < ")
                .push_bind(id.to_string())
                .push("))");
        }
        if let Some(query) = filter.query.as_deref().filter(|q| !q.trim().is_empty()) {
            let pattern = format!("%{}%", like_escape(query.trim()));
            qb.push(" AND (keyword_ko LIKE ")
                .push_bind(pattern.clone())
                .push(" ESCAPE '\\' OR prompt_final LIKE ")
                .push_bind(pattern)
                .push(" ESCAPE '\\')");
        }
        if let Some(style_id) = &filter.style_id {
            qb.push(" AND style_id = ").push_bind(style_id.clone());
        }
        if filter.favorite == Some(true) {
            qb.push(" AND favorite = 1");
        }
        qb.push(" ORDER BY created_at DESC, id DESC LIMIT ")
            .push_bind(limit);
        qb.build_query_as::<Generation>()
            .fetch_all(&self.pool)
            .await
    }

    pub async fn set_favorite(&self, id: &str, favorite: bool) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE generations SET favorite = ? WHERE id = ?")
            .bind(favorite)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // --- training_jobs ---

    pub async fn insert_training_job(&self, job: &TrainingJob) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO training_jobs (
                id, style_id, status, progress, eta_seconds, params_json, error, started_at, finished_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&job.id)
        .bind(&job.style_id)
        .bind(&job.status)
        .bind(job.progress)
        .bind(job.eta_seconds)
        .bind(&job.params_json)
        .bind(&job.error)
        .bind(job.started_at)
        .bind(job.finished_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_training_job(&self, id: &str) -> Result<Option<TrainingJob>, sqlx::Error> {
        sqlx::query_as::<_, TrainingJob>("SELECT * FROM training_jobs WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn update_training_progress(
        &self,
        id: &str,
        status: &str,
        progress: f64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE training_jobs SET status = ?, progress = ? WHERE id = ?")
            .bind(status)
            .bind(progress)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::models::{Generation, TrainingJob};
    use super::{like_escape, Db, HistoryFilter};

    fn sample_generation(id: &str, created_at: i64) -> Generation {
        Generation {
            id: id.to_string(),
            created_at,
            image_path: "outputs/2026-07/a.png".to_string(),
            thumb_path: "outputs/2026-07/a.thumb.png".to_string(),
            keyword_ko: Some("통나무집".to_string()),
            prompt_final: "cinematic illustration of a log cabin".to_string(),
            negative: Some("text, watermark".to_string()),
            preset_id: Some("storybook".to_string()),
            preset_version: Some(3),
            style_id: None,
            seed: 42,
            steps: Some(28),
            cfg: Some(6.5),
            width: Some(1024),
            height: Some(1024),
            model: Some("sdxl".to_string()),
            favorite: false,
        }
    }

    #[tokio::test]
    async fn generation_crud_roundtrip() {
        let db = Db::connect_in_memory().await.unwrap();

        let g = sample_generation("g1", 1000);
        db.insert_generation(&g).await.unwrap();

        // read: 전 필드 왕복 일치
        let fetched = db.get_generation("g1").await.unwrap().unwrap();
        assert_eq!(fetched, g);

        // 없는 id
        assert!(db.get_generation("nope").await.unwrap().is_none());

        // favorite update
        db.set_favorite("g1", true).await.unwrap();
        assert!(db.get_generation("g1").await.unwrap().unwrap().favorite);
    }

    #[tokio::test]
    async fn list_generations_orders_by_created_desc() {
        let db = Db::connect_in_memory().await.unwrap();
        db.insert_generation(&sample_generation("old", 100))
            .await
            .unwrap();
        db.insert_generation(&sample_generation("new", 200))
            .await
            .unwrap();

        let list = db.list_generations(10).await.unwrap();
        let ids: Vec<_> = list.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(ids, vec!["new", "old"]);

        // limit 반영
        assert_eq!(db.list_generations(1).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_page_keyset_pagination_is_stable() {
        let db = Db::connect_in_memory().await.unwrap();
        // created_at 충돌 케이스 포함: (300, c) (200, b2) (200, b1) (100, a)
        for (id, at) in [("a", 100), ("b1", 200), ("b2", 200), ("c", 300)] {
            db.insert_generation(&sample_generation(id, at))
                .await
                .unwrap();
        }

        // 1페이지
        let p1 = db
            .list_generations_page(2, None, &HistoryFilter::default())
            .await
            .unwrap();
        let ids1: Vec<_> = p1.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(ids1, vec!["c", "b2"]);

        // 2페이지: 커서 = 1페이지 마지막 (200, b2) — 같은 created_at의 b1이 빠지면 안 됨
        let last = p1.last().unwrap();
        let p2 = db
            .list_generations_page(
                2,
                Some((last.created_at, &last.id)),
                &HistoryFilter::default(),
            )
            .await
            .unwrap();
        let ids2: Vec<_> = p2.iter().map(|g| g.id.as_str()).collect();
        assert_eq!(ids2, vec!["b1", "a"]);

        // 끝: 빈 페이지
        let last2 = p2.last().unwrap();
        let p3 = db
            .list_generations_page(
                2,
                Some((last2.created_at, &last2.id)),
                &HistoryFilter::default(),
            )
            .await
            .unwrap();
        assert!(p3.is_empty());
    }

    #[tokio::test]
    async fn list_page_filters_query_favorite_style() {
        let db = Db::connect_in_memory().await.unwrap();
        let mut cabin = sample_generation("cabin", 300); // 통나무집 / log cabin
        cabin.favorite = true;
        db.insert_generation(&cabin).await.unwrap();

        let mut robot = sample_generation("robot", 200);
        robot.keyword_ko = Some("로봇".to_string());
        robot.prompt_final = "cute 3d render of, robot".to_string();
        robot.style_id = Some("style-1".to_string());
        db.insert_generation(&robot).await.unwrap();

        let all = HistoryFilter::default();
        assert_eq!(
            db.list_generations_page(10, None, &all)
                .await
                .unwrap()
                .len(),
            2
        );

        // query: 키워드(한글) 또는 프롬프트(영문) 부분 일치
        for (q, expect) in [("통나무", "cabin"), ("robot", "robot"), ("cabin", "cabin")] {
            let f = HistoryFilter {
                query: Some(q.to_string()),
                ..Default::default()
            };
            let hits = db.list_generations_page(10, None, &f).await.unwrap();
            assert_eq!(hits.len(), 1, "query={q}");
            assert_eq!(hits[0].id, expect, "query={q}");
        }

        // LIKE 와일드카드가 이스케이프되는지 — '%'로는 전부 매칭되면 안 됨
        let wild = HistoryFilter {
            query: Some("%".to_string()),
            ..Default::default()
        };
        assert!(db
            .list_generations_page(10, None, &wild)
            .await
            .unwrap()
            .is_empty());

        // favorite
        let fav = HistoryFilter {
            favorite: Some(true),
            ..Default::default()
        };
        let hits = db.list_generations_page(10, None, &fav).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "cabin");

        // style_id
        let style = HistoryFilter {
            style_id: Some("style-1".to_string()),
            ..Default::default()
        };
        let hits = db.list_generations_page(10, None, &style).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "robot");

        // 필터 + 커서 조합 (favorite 안에서 keyset이 동작)
        let mut cabin2 = sample_generation("cabin2", 100);
        cabin2.favorite = true;
        db.insert_generation(&cabin2).await.unwrap();
        let p1 = db.list_generations_page(1, None, &fav).await.unwrap();
        assert_eq!(p1[0].id, "cabin");
        let p2 = db
            .list_generations_page(1, Some((p1[0].created_at, &p1[0].id)), &fav)
            .await
            .unwrap();
        assert_eq!(p2[0].id, "cabin2");
    }

    #[test]
    fn like_escape_escapes_wildcards() {
        assert_eq!(like_escape("100%_\\"), "100\\%\\_\\\\");
        assert_eq!(like_escape("통나무집"), "통나무집");
    }

    #[tokio::test]
    async fn training_job_crud() {
        let db = Db::connect_in_memory().await.unwrap();
        let job = TrainingJob {
            id: "t1".to_string(),
            style_id: "s1".to_string(),
            status: "queued".to_string(),
            progress: 0.0,
            eta_seconds: None,
            params_json: Some("{}".to_string()),
            error: None,
            started_at: None,
            finished_at: None,
        };
        db.insert_training_job(&job).await.unwrap();
        assert_eq!(db.get_training_job("t1").await.unwrap().unwrap(), job);

        db.update_training_progress("t1", "training", 0.5)
            .await
            .unwrap();
        let updated = db.get_training_job("t1").await.unwrap().unwrap();
        assert_eq!(updated.status, "training");
        assert_eq!(updated.progress, 0.5);
    }
}
