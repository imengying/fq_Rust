use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub fq: FqConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct FqConfig {
    pub upstream: UpstreamConfig,
    pub signer: SignerConfig,
    pub cache: CacheConfig,
    pub prefetch: PrefetchConfig,
    pub search: SearchConfig,
    pub auto_heal: AutoHealConfig,
    pub device_rotate_cooldown_ms: u64,
    pub device_pool_probe_on_startup: bool,
    pub device_pool_probe_max_attempts: usize,
    pub device_pool_startup_name: Option<String>,
    pub device_pool: Vec<DeviceProfile>,
    pub device_profile: DeviceProfile,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct UpstreamConfig {
    pub base_url: String,
    pub search_base_url: Option<String>,
    pub connect_timeout_ms: u64,
    pub read_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SignerConfig {
    pub backend: SignerBackendKind,
    pub command: Vec<String>,
    pub restart_cooldown_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SignerBackendKind {
    JavaWorker,
    RustNative,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct CacheConfig {
    pub search_ttl_ms: u64,
    pub directory_ttl_ms: u64,
    pub book_ttl_ms: u64,
    pub chapter_ttl_ms: u64,
    pub register_key_ttl_ms: u64,
    pub register_key_max_entries: u64,
    pub postgres_url: Option<String>,
    pub postgres_table: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct PrefetchConfig {
    pub enabled: bool,
    pub chapter_size: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SearchConfig {
    pub phase1_delay_min_ms: u64,
    pub phase1_delay_max_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AutoHealConfig {
    pub enabled: bool,
    pub error_threshold: usize,
    pub window_ms: u64,
    pub cooldown_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct DeviceProfile {
    pub name: String,
    pub user_agent: String,
    pub cookie: String,
    pub device: UpstreamDevice,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct UpstreamDevice {
    pub aid: String,
    pub cdid: String,
    pub device_id: String,
    pub device_type: String,
    pub device_brand: String,
    pub install_id: String,
    pub resolution: String,
    pub dpi: String,
    pub rom_version: String,
    pub host_abi: String,
    pub update_version_code: String,
    pub version_code: String,
    pub version_name: String,
    pub os_version: String,
    pub os_api: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            fq: FqConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 9999,
        }
    }
}

impl Default for FqConfig {
    fn default() -> Self {
        Self {
            upstream: UpstreamConfig::default(),
            signer: SignerConfig::default(),
            cache: CacheConfig::default(),
            prefetch: PrefetchConfig::default(),
            search: SearchConfig::default(),
            auto_heal: AutoHealConfig::default(),
            device_rotate_cooldown_ms: 60_000,
            device_pool_probe_on_startup: false,
            device_pool_probe_max_attempts: 3,
            device_pool_startup_name: None,
            device_pool: Vec::new(),
            device_profile: DeviceProfile::default(),
        }
    }
}

impl Default for UpstreamConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api5-normal-sinfonlineb.fqnovel.com".to_string(),
            search_base_url: Some("https://api5-normal-sinfonlinec.fqnovel.com".to_string()),
            connect_timeout_ms: 15_000,
            read_timeout_ms: 30_000,
        }
    }
}

impl Default for SignerConfig {
    fn default() -> Self {
        Self {
            backend: SignerBackendKind::RustNative,
            command: default_signer_command(SignerBackendKind::RustNative),
            restart_cooldown_ms: 2_000,
        }
    }
}

impl Default for SignerBackendKind {
    fn default() -> Self {
        Self::RustNative
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            search_ttl_ms: 45_000,
            directory_ttl_ms: 600_000,
            book_ttl_ms: 600_000,
            chapter_ttl_ms: 600_000,
            register_key_ttl_ms: 3_600_000,
            register_key_max_entries: 128,
            postgres_url: None,
            postgres_table: "fq_chapter_cache".to_string(),
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            phase1_delay_min_ms: 1_000,
            phase1_delay_max_ms: 2_000,
        }
    }
}

impl Default for PrefetchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            chapter_size: 30,
        }
    }
}

impl Default for AutoHealConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            error_threshold: 5,
            window_ms: 300_000,
            cooldown_ms: 180_000,
        }
    }
}

impl Default for DeviceProfile {
    fn default() -> Self {
        Self {
            name: "dev01".to_string(),
            user_agent: "com.dragon.read.oversea.gp/68132 (Linux; U; Android 13; zh_CN; Sirius; Build/V417IR;tt-ok/3.12.13.4-tiktok)".to_string(),
            cookie: "store-region=cn-zj; store-region-src=did; install_id=573270579220059".to_string(),
            device: UpstreamDevice::default(),
        }
    }
}

impl Default for UpstreamDevice {
    fn default() -> Self {
        Self {
            aid: "1967".to_string(),
            cdid: "9daf93bf-4dcf-417e-8795-20284ad26a1f".to_string(),
            device_id: "1778337441136410".to_string(),
            device_type: "Sirius".to_string(),
            device_brand: "Xiaomi".to_string(),
            install_id: "573270579220059".to_string(),
            resolution: "2244*1080".to_string(),
            dpi: "440".to_string(),
            rom_version: "V417IR+release-keys".to_string(),
            host_abi: "arm64-v8a".to_string(),
            update_version_code: "68132".to_string(),
            version_code: "68132".to_string(),
            version_name: "6.8.1.32".to_string(),
            os_version: "13".to_string(),
            os_api: "33".to_string(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let mut config = load_from_disk()?;
        config.apply_env();
        config.normalize_signer_command();
        config.inherit_device_pool_defaults();
        config.validate()?;
        Ok(config)
    }

    fn apply_env(&mut self) {
        set_string(&mut self.server.host, "FQRS_SERVER_HOST");
        set_u16(&mut self.server.port, "FQRS_SERVER_PORT");
        set_string(&mut self.fq.upstream.base_url, "FQRS_UPSTREAM_BASE_URL");
        set_optional_string(
            &mut self.fq.upstream.search_base_url,
            "FQRS_UPSTREAM_SEARCH_BASE_URL",
        );
        set_u64(
            &mut self.fq.upstream.connect_timeout_ms,
            "FQRS_UPSTREAM_CONNECT_TIMEOUT_MS",
        );
        set_u64(
            &mut self.fq.upstream.read_timeout_ms,
            "FQRS_UPSTREAM_READ_TIMEOUT_MS",
        );
        set_command(&mut self.fq.signer.command, "FQRS_SIGNER_COMMAND");
        set_signer_backend(&mut self.fq.signer.backend, "FQRS_SIGNER_BACKEND");
        set_u64(
            &mut self.fq.signer.restart_cooldown_ms,
            "FQRS_SIGNER_RESTART_COOLDOWN_MS",
        );
        set_u64(
            &mut self.fq.cache.book_ttl_ms,
            "FQRS_BOOK_CACHE_TTL_MS",
        );
        set_bool(&mut self.fq.prefetch.enabled, "FQRS_PREFETCH_ENABLED");
        set_usize(
            &mut self.fq.prefetch.chapter_size,
            "FQRS_PREFETCH_CHAPTER_SIZE",
        );
        set_u64(
            &mut self.fq.cache.register_key_ttl_ms,
            "FQRS_REGISTER_KEY_CACHE_TTL_MS",
        );
        set_u64(
            &mut self.fq.cache.register_key_max_entries,
            "FQRS_REGISTER_KEY_CACHE_MAX_ENTRIES",
        );
        set_optional_string(&mut self.fq.cache.postgres_url, "FQRS_DB_URL");
        if self.fq.cache.postgres_url.is_none() {
            set_optional_string(&mut self.fq.cache.postgres_url, "DB_URL");
        }
        set_string(&mut self.fq.cache.postgres_table, "FQRS_DB_TABLE");
        set_u64(
            &mut self.fq.device_rotate_cooldown_ms,
            "FQRS_DEVICE_ROTATE_COOLDOWN_MS",
        );
        set_bool(
            &mut self.fq.device_pool_probe_on_startup,
            "FQRS_DEVICE_POOL_PROBE_ON_STARTUP",
        );
        set_usize(
            &mut self.fq.device_pool_probe_max_attempts,
            "FQRS_DEVICE_POOL_PROBE_MAX_ATTEMPTS",
        );
        set_bool(&mut self.fq.auto_heal.enabled, "FQRS_AUTO_HEAL_ENABLED");
        set_usize(
            &mut self.fq.auto_heal.error_threshold,
            "FQRS_AUTO_HEAL_ERROR_THRESHOLD",
        );
        set_u64(&mut self.fq.auto_heal.window_ms, "FQRS_AUTO_HEAL_WINDOW_MS");
        set_u64(
            &mut self.fq.auto_heal.cooldown_ms,
            "FQRS_AUTO_HEAL_COOLDOWN_MS",
        );
    }

    fn validate(&self) -> Result<()> {
        if self.server.port == 0 {
            return Err(anyhow!("server.port 不能为空"));
        }
        if self.fq.signer.command.is_empty() {
            return Err(anyhow!("fq.signer.command 不能为空"));
        }
        if self.fq.cache.postgres_table.trim().is_empty() {
            return Err(anyhow!("fq.cache.postgres_table 不能为空"));
        }
        validate_device_profile(&self.fq.device_profile, "fq.device_profile")?;
        for (index, profile) in self.fq.device_pool.iter().enumerate() {
            validate_device_profile(profile, &format!("fq.device_pool[{index}]"))?;
        }
        Ok(())
    }

    fn inherit_device_pool_defaults(&mut self) {
        let Some(bootstrap) = self.fq.resolve_bootstrap_profile().cloned() else {
            return;
        };

        if self.fq.device_profile == DeviceProfile::default() {
            self.fq.device_profile = bootstrap;
            return;
        }

        self.fq.device_profile.inherit_missing_from(&bootstrap);
    }

    fn normalize_signer_command(&mut self) {
        let java_default = default_signer_command(SignerBackendKind::JavaWorker);
        let rust_default = default_signer_command(SignerBackendKind::RustNative);

        if self.fq.signer.command.is_empty() {
            self.fq.signer.command = default_signer_command(self.fq.signer.backend.clone());
            return;
        }

        match self.fq.signer.backend {
            SignerBackendKind::JavaWorker if self.fq.signer.command == rust_default => {
                self.fq.signer.command = java_default;
            }
            SignerBackendKind::RustNative if self.fq.signer.command == java_default => {
                self.fq.signer.command = rust_default;
            }
            _ => {}
        }
    }
}

impl UpstreamConfig {
    pub fn resolved_search_base_url(&self) -> String {
        if let Some(search_base_url) = &self.search_base_url {
            if !search_base_url.trim().is_empty() {
                return search_base_url.clone();
            }
        }
        self.base_url
            .replace("api5-normal-sinfonlineb", "api5-normal-sinfonlinec")
    }
}

impl FqConfig {
    fn resolve_bootstrap_profile(&self) -> Option<&DeviceProfile> {
        if self.device_pool.is_empty() {
            return None;
        }

        if let Some(startup_name) = self
            .device_pool_startup_name
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            if let Some(profile) = self
                .device_pool
                .iter()
                .find(|profile| profile.name.trim() == startup_name)
            {
                return Some(profile);
            }
        }

        self.device_pool.first()
    }
}

impl DeviceProfile {
    fn inherit_missing_from(&mut self, fallback: &DeviceProfile) {
        inherit_string(&mut self.name, &fallback.name);
        inherit_string(&mut self.user_agent, &fallback.user_agent);
        inherit_string(&mut self.cookie, &fallback.cookie);
        self.device.inherit_missing_from(&fallback.device);
    }
}

impl UpstreamDevice {
    fn inherit_missing_from(&mut self, fallback: &UpstreamDevice) {
        inherit_string(&mut self.aid, &fallback.aid);
        inherit_string(&mut self.cdid, &fallback.cdid);
        inherit_string(&mut self.device_id, &fallback.device_id);
        inherit_string(&mut self.device_type, &fallback.device_type);
        inherit_string(&mut self.device_brand, &fallback.device_brand);
        inherit_string(&mut self.install_id, &fallback.install_id);
        inherit_string(&mut self.resolution, &fallback.resolution);
        inherit_string(&mut self.dpi, &fallback.dpi);
        inherit_string(&mut self.rom_version, &fallback.rom_version);
        inherit_string(&mut self.host_abi, &fallback.host_abi);
        inherit_string(&mut self.update_version_code, &fallback.update_version_code);
        inherit_string(&mut self.version_code, &fallback.version_code);
        inherit_string(&mut self.version_name, &fallback.version_name);
        inherit_string(&mut self.os_version, &fallback.os_version);
        inherit_string(&mut self.os_api, &fallback.os_api);
    }
}

fn validate_device_profile(profile: &DeviceProfile, field_name: &str) -> Result<()> {
    if profile.user_agent.trim().is_empty() {
        return Err(anyhow!("{field_name}.user_agent 不能为空"));
    }
    if profile.cookie.trim().is_empty() {
        return Err(anyhow!("{field_name}.cookie 不能为空"));
    }
    if profile.device.device_id.trim().is_empty() {
        return Err(anyhow!("{field_name}.device.device_id 不能为空"));
    }
    Ok(())
}

fn inherit_string(target: &mut String, fallback: &str) {
    if target.trim().is_empty() {
        *target = fallback.to_string();
    }
}

fn load_from_disk() -> Result<AppConfig> {
    if let Ok(custom_path) = env::var("FQRS_CONFIG_PATH") {
        let custom_path = custom_path.trim();
        if !custom_path.is_empty() && Path::new(custom_path).exists() {
            let file = File::open(custom_path)?;
            let config = serde_yaml::from_reader(file)?;
            return Ok(config);
        }
    }

    const PATHS: [&str; 1] = ["configs/config.yaml"];

    for path in PATHS {
        if Path::new(path).exists() {
            let file = File::open(path)?;
            let config = serde_yaml::from_reader(file)?;
            return Ok(config);
        }
    }

    Ok(AppConfig::default())
}

fn set_string(target: &mut String, key: &str) {
    if let Ok(value) = env::var(key) {
        if !value.trim().is_empty() {
            *target = value;
        }
    }
}

fn set_optional_string(target: &mut Option<String>, key: &str) {
    if let Ok(value) = env::var(key) {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            *target = None;
        } else {
            *target = Some(trimmed.to_string());
        }
    }
}

fn set_u64(target: &mut u64, key: &str) {
    if let Ok(value) = env::var(key) {
        if let Ok(parsed) = value.parse::<u64>() {
            *target = parsed;
        }
    }
}

fn set_u16(target: &mut u16, key: &str) {
    if let Ok(value) = env::var(key) {
        if let Ok(parsed) = value.parse::<u16>() {
            *target = parsed;
        }
    }
}

fn set_usize(target: &mut usize, key: &str) {
    if let Ok(value) = env::var(key) {
        if let Ok(parsed) = value.parse::<usize>() {
            *target = parsed;
        }
    }
}

fn set_bool(target: &mut bool, key: &str) {
    if let Ok(value) = env::var(key) {
        match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => *target = true,
            "0" | "false" | "no" | "off" => *target = false,
            _ => {}
        }
    }
}

fn set_signer_backend(target: &mut SignerBackendKind, key: &str) {
    if let Ok(value) = env::var(key) {
        match value.trim().to_ascii_lowercase().as_str() {
            "java_worker" | "java-worker" | "java" => *target = SignerBackendKind::JavaWorker,
            "rust_native" | "rust-native" | "rust" => *target = SignerBackendKind::RustNative,
            _ => {}
        }
    }
}

fn set_command(target: &mut Vec<String>, key: &str) {
    if let Ok(value) = env::var(key) {
        let parsed: Vec<String> = value
            .split_whitespace()
            .filter(|item| !item.trim().is_empty())
            .map(ToString::to_string)
            .collect();
        if !parsed.is_empty() {
            *target = parsed;
        }
    }
}

fn default_signer_command(backend: SignerBackendKind) -> Vec<String> {
    match backend {
        SignerBackendKind::JavaWorker => vec![
            "java".to_string(),
            "--enable-native-access=ALL-UNNAMED".to_string(),
            "-jar".to_string(),
            "/app/fq-signer.jar".to_string(),
        ],
        SignerBackendKind::RustNative => vec!["/app/fq-signer-native".to_string()],
    }
}
