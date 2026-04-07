use crate::auto_heal::AutoHealManager;
use crate::cache::TtlCache;
use crate::config::AppConfig;
use crate::db_cache::PgChapterCache;
use crate::device_pool::DevicePoolManager;
use crate::models::{BookInfo, ChapterInfo, DirectoryResponse, SearchResponse};
use crate::registerkey::RegisterKeyService;
use crate::signer::SignerClient;
use anyhow::{anyhow, Result};
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub auto_heal: AutoHealManager,
    pub device_pool: DevicePoolManager,
    pub http_client: reqwest::Client,
    pub signer_client: SignerClient,
    pub search_cache: TtlCache<SearchResponse>,
    pub directory_cache: TtlCache<DirectoryResponse>,
    pub book_cache: TtlCache<BookInfo>,
    pub chapter_cache: TtlCache<ChapterInfo>,
    pub pg_chapter_cache: Option<PgChapterCache>,
    pub register_key_service: RegisterKeyService,
}

impl AppState {
    pub async fn new(config: AppConfig) -> Result<Arc<Self>> {
        let http_client = reqwest::Client::builder()
            .http1_only()
            .no_gzip()
            .connect_timeout(Duration::from_millis(
                config.fq.upstream.connect_timeout_ms,
            ))
            .timeout(Duration::from_millis(config.fq.upstream.read_timeout_ms))
            .build()?;

        let search_ttl = duration_from_ms(config.fq.cache.search_ttl_ms);
        let directory_ttl = duration_from_ms(config.fq.cache.directory_ttl_ms);
        let chapter_ttl = duration_from_ms(config.fq.cache.chapter_ttl_ms);
        let device_pool = DevicePoolManager::new(
            config.fq.device_profile.clone(),
            config.fq.device_pool.clone(),
            config.fq.device_pool_startup_name.clone(),
            config.fq.device_rotate_cooldown_ms,
        );
        let auto_heal = AutoHealManager::new(
            config.fq.auto_heal.enabled,
            config.fq.auto_heal.error_threshold,
            config.fq.auto_heal.window_ms,
            config.fq.auto_heal.cooldown_ms,
        );
        let pg_chapter_cache = if let Some(database_url) = config
            .fq
            .cache
            .postgres_url
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            match PgChapterCache::new(
                database_url,
                &config.fq.cache.postgres_table,
                config.fq.cache.chapter_ttl_ms,
            )
            .await
            {
                Ok(cache) => Some(cache),
                Err(error) => {
                    warn!("postgres chapter cache disabled: {error}");
                    None
                }
            }
        } else {
            None
        };
        let config = Arc::new(config);

        Ok(Arc::new(Self {
            auto_heal,
            device_pool,
            signer_client: SignerClient::new(config.fq.signer.clone())
                .map_err(|error| anyhow!(error.message))?,
            http_client,
            search_cache: TtlCache::new(search_ttl),
            directory_cache: TtlCache::new(directory_ttl),
            book_cache: TtlCache::new(directory_ttl),
            chapter_cache: TtlCache::new(chapter_ttl),
            pg_chapter_cache,
            register_key_service: RegisterKeyService::new(
                config.fq.cache.register_key_ttl_ms,
                config.fq.cache.register_key_max_entries as usize,
            ),
            config,
        }))
    }

    pub fn current_device_profile(&self) -> crate::config::DeviceProfile {
        self.device_pool.current_profile()
    }

    pub fn rotate_device_if_allowed(&self, reason: &str) -> bool {
        self.device_pool.rotate_if_allowed(reason)
    }

    pub fn record_success(&self) {
        self.auto_heal.record_success();
    }

    pub fn record_failure_and_should_heal(&self) -> bool {
        self.auto_heal.record_failure_and_should_heal()
    }
}

fn duration_from_ms(value: u64) -> Option<Duration> {
    if value == 0 {
        None
    } else {
        Some(Duration::from_millis(value))
    }
}
