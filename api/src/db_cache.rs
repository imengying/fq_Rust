use crate::models::ChapterInfo;
use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres, Row};
use std::time::Duration;

#[derive(Clone)]
pub struct PgChapterCache {
    pool: Pool<Postgres>,
    table_name: String,
    ttl_ms: u64,
}

impl PgChapterCache {
    pub async fn new(database_url: &str, table_name: &str, ttl_ms: u64) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(3)
            .acquire_timeout(Duration::from_secs(5))
            .connect(database_url)
            .await?;

        let sanitized_table = sanitize_table_name(table_name);
        let create_sql = format!(
            "CREATE TABLE IF NOT EXISTS {sanitized_table} (
                cache_key TEXT PRIMARY KEY,
                payload_json TEXT NOT NULL,
                created_at_ms BIGINT NOT NULL
            )"
        );
        sqlx::query(&create_sql).execute(&pool).await?;

        Ok(Self {
            pool,
            table_name: sanitized_table,
            ttl_ms,
        })
    }

    pub async fn get(&self, cache_key: &str, now_ms: i64) -> Result<Option<ChapterInfo>> {
        let sql = format!(
            "SELECT payload_json, created_at_ms FROM {} WHERE cache_key = $1",
            self.table_name
        );
        let row = sqlx::query(&sql)
            .bind(cache_key)
            .fetch_optional(&self.pool)
            .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let created_at_ms: i64 = row.try_get("created_at_ms")?;
        if self.ttl_ms > 0 && now_ms.saturating_sub(created_at_ms) > self.ttl_ms as i64 {
            self.delete(cache_key).await?;
            return Ok(None);
        }

        let payload_json: String = row.try_get("payload_json")?;
        let parsed = serde_json::from_str(&payload_json)?;
        Ok(Some(parsed))
    }

    pub async fn put(&self, cache_key: &str, chapter: &ChapterInfo, now_ms: i64) -> Result<()> {
        if chapter.txt_content.trim().is_empty() {
            return Ok(());
        }

        let payload_json = serde_json::to_string(chapter)?;
        let sql = format!(
            "INSERT INTO {} (cache_key, payload_json, created_at_ms)
             VALUES ($1, $2, $3)
             ON CONFLICT (cache_key)
             DO UPDATE SET payload_json = EXCLUDED.payload_json, created_at_ms = EXCLUDED.created_at_ms",
            self.table_name
        );
        sqlx::query(&sql)
            .bind(cache_key)
            .bind(payload_json)
            .bind(now_ms)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete(&self, cache_key: &str) -> Result<()> {
        let sql = format!("DELETE FROM {} WHERE cache_key = $1", self.table_name);
        sqlx::query(&sql).bind(cache_key).execute(&self.pool).await?;
        Ok(())
    }
}

fn sanitize_table_name(value: &str) -> String {
    let trimmed = value.trim();
    let valid = trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    if trimmed.is_empty() || !valid {
        "fq_chapter_cache".to_string()
    } else {
        trimmed.to_string()
    }
}
