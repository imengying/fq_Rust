use crate::cache::TtlCache;
use crate::config::AppConfig;
use crate::models::{BookInfo, ChapterInfo, DirectoryResponse, SearchResponse};
use crate::sidecar::SidecarClient;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub http_client: reqwest::Client,
    pub sidecar_client: SidecarClient,
    pub search_cache: TtlCache<SearchResponse>,
    pub directory_cache: TtlCache<DirectoryResponse>,
    pub book_cache: TtlCache<BookInfo>,
    pub chapter_cache: TtlCache<ChapterInfo>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Result<Arc<Self>> {
        let http_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(
                config.fq.upstream.connect_timeout_ms,
            ))
            .timeout(Duration::from_millis(config.fq.upstream.read_timeout_ms))
            .build()?;

        let sidecar_http = reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(config.fq.sidecar.timeout_ms))
            .timeout(Duration::from_millis(config.fq.sidecar.timeout_ms))
            .build()?;

        let search_ttl = duration_from_ms(config.fq.cache.search_ttl_ms);
        let directory_ttl = duration_from_ms(config.fq.cache.directory_ttl_ms);
        let chapter_ttl = duration_from_ms(config.fq.cache.chapter_ttl_ms);
        let config = Arc::new(config);

        Ok(Arc::new(Self {
            sidecar_client: SidecarClient::new(
                sidecar_http,
                config.fq.sidecar.base_url.clone(),
                config.fq.sidecar.internal_token.clone(),
            ),
            http_client,
            search_cache: TtlCache::new(search_ttl),
            directory_cache: TtlCache::new(directory_ttl),
            book_cache: TtlCache::new(directory_ttl),
            chapter_cache: TtlCache::new(chapter_ttl),
            config,
        }))
    }
}

fn duration_from_ms(value: u64) -> Option<Duration> {
    if value == 0 {
        None
    } else {
        Some(Duration::from_millis(value))
    }
}
