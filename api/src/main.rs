mod auto_heal;
mod cache;
mod config;
mod content;
mod db_cache;
mod device_pool;
mod encoding;
mod fq;
mod models;
mod registerkey;
mod signer;
mod state;
mod upstream;

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use models::{ApiResponse, ServiceError};
use serde::Deserialize;
use state::AppState;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let config = config::AppConfig::load()?;
    let state = AppState::new(config.clone()).await?;
    upstream::run_startup_probe(&state).await;
    let app = Router::new()
        .route("/search", get(search))
        .route("/book/{book_id}", get(book))
        .route("/toc/{book_id}", get(toc))
        .route("/chapter/{book_id}/{chapter_id}", get(chapter))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let address: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    tracing::info!("fq-api listening on {address}");

    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SearchQuery>,
) -> Json<ApiResponse<models::SearchResponse>> {
    match validate_search_query(query) {
        Ok(validated) => match upstream::search_books(
            &state,
            validated.key,
            validated.page,
            validated.size,
            validated.tab_type,
            validated.search_id,
        )
        .await
        {
            Ok(response) => Json(ApiResponse::success(response)),
            Err(error) => Json(ApiResponse::error(error.code, error.message)),
        },
        Err(error) => Json(ApiResponse::error(error.code, error.message)),
    }
}

async fn book(
    State(state): State<Arc<AppState>>,
    Path(book_id): Path<String>,
) -> Json<ApiResponse<models::BookInfo>> {
    match validate_numeric_id(&book_id, "书籍ID") {
        Ok(book_id) => match upstream::get_book_info(&state, &book_id).await {
            Ok(response) => Json(ApiResponse::success(response)),
            Err(error) => Json(ApiResponse::error(error.code, error.message)),
        },
        Err(error) => Json(ApiResponse::error(error.code, error.message)),
    }
}

async fn toc(
    State(state): State<Arc<AppState>>,
    Path(book_id): Path<String>,
) -> Json<ApiResponse<models::DirectoryResponse>> {
    match validate_numeric_id(&book_id, "书籍ID") {
        Ok(book_id) => match upstream::get_toc(&state, &book_id).await {
            Ok(response) => Json(ApiResponse::success(response)),
            Err(error) => Json(ApiResponse::error(error.code, error.message)),
        },
        Err(error) => Json(ApiResponse::error(error.code, error.message)),
    }
}

async fn chapter(
    State(state): State<Arc<AppState>>,
    Path((book_id, chapter_id)): Path<(String, String)>,
) -> Json<ApiResponse<models::ChapterInfo>> {
    match validate_numeric_id(&book_id, "书籍ID")
        .and_then(|book_id| validate_numeric_id(&chapter_id, "章节ID").map(|chapter_id| (book_id, chapter_id)))
    {
        Ok((book_id, chapter_id)) => match upstream::get_chapter(&state, &book_id, &chapter_id).await {
            Ok(response) => Json(ApiResponse::success(response)),
            Err(error) => Json(ApiResponse::error(error.code, error.message)),
        },
        Err(error) => Json(ApiResponse::error(error.code, error.message)),
    }
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    key: String,
    page: Option<usize>,
    size: Option<usize>,
    #[serde(rename = "tabType")]
    tab_type: Option<u32>,
    #[serde(rename = "searchId")]
    search_id: Option<String>,
}

struct ValidatedSearchQuery {
    key: String,
    page: usize,
    size: usize,
    tab_type: u32,
    search_id: Option<String>,
}

fn validate_search_query(query: SearchQuery) -> Result<ValidatedSearchQuery, ServiceError> {
    let key = query.key.trim().to_string();
    if key.is_empty() {
        return Err(ServiceError::bad_request("搜索关键词不能为空"));
    }
    if key.chars().count() > 100 {
        return Err(ServiceError::bad_request("搜索关键词过长"));
    }
    let page = query.page.unwrap_or(1);
    if page == 0 {
        return Err(ServiceError::bad_request("页码必须大于等于1"));
    }
    let size = query.size.unwrap_or(20);
    if !(1..=50).contains(&size) {
        return Err(ServiceError::bad_request("size 超出范围（1-50）"));
    }
    let tab_type = query.tab_type.unwrap_or(3);
    if !(1..=20).contains(&tab_type) {
        return Err(ServiceError::bad_request("tabType 超出范围"));
    }

    Ok(ValidatedSearchQuery {
        key,
        page,
        size,
        tab_type,
        search_id: query.search_id.and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }),
    })
}

fn validate_numeric_id(value: &str, field_name: &str) -> Result<String, ServiceError> {
    let normalized = value.trim();
    if normalized.is_empty() || !normalized.chars().all(|value| value.is_ascii_digit()) {
        return Err(ServiceError::bad_request(format!("{field_name}必须为纯数字")));
    }
    Ok(normalized.to_string())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}

fn init_tracing() {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tower_http=info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .init();
}
