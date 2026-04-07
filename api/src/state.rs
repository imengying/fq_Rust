use crate::cache::TtlCache;
use crate::config::AppConfig;
use crate::models::{BookInfo, ChapterInfo, DirectoryResponse, SearchResponse};
use crate::registerkey::RegisterKeyService;
use crate::signer::SignerClient;
use anyhow::{anyhow, Result};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub http_client: reqwest::Client,
    pub signer_client: SignerClient,
    pub search_cache: TtlCache<SearchResponse>,
    pub directory_cache: TtlCache<DirectoryResponse>,
    pub book_cache: TtlCache<BookInfo>,
    pub chapter_cache: TtlCache<ChapterInfo>,
    pub register_key_service: RegisterKeyService,
}

impl AppState {
    pub fn new(config: AppConfig) -> Result<Arc<Self>> {
        let http_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(
                config.fq.upstream.connect_timeout_ms,
            ))
            .timeout(Duration::from_millis(config.fq.upstream.read_timeout_ms))
            .build()?;

        let search_ttl = duration_from_ms(config.fq.cache.search_ttl_ms);
        let directory_ttl = duration_from_ms(config.fq.cache.directory_ttl_ms);
        let chapter_ttl = duration_from_ms(config.fq.cache.chapter_ttl_ms);
        let config = Arc::new(config);

        Ok(Arc::new(Self {
            signer_client: SignerClient::new(config.fq.signer.clone())
                .map_err(|error| anyhow!(error.message))?,
            http_client,
            search_cache: TtlCache::new(search_ttl),
            directory_cache: TtlCache::new(directory_ttl),
            book_cache: TtlCache::new(directory_ttl),
            chapter_cache: TtlCache::new(chapter_ttl),
            register_key_service: RegisterKeyService::new(
                config.fq.cache.register_key_ttl_ms,
                config.fq.cache.register_key_max_entries as usize,
            ),
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
