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
    pub sidecar: SidecarConfig,
    pub cache: CacheConfig,
    pub search: SearchConfig,
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
pub struct SidecarConfig {
    pub command: Vec<String>,
    pub restart_cooldown_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct CacheConfig {
    pub search_ttl_ms: u64,
    pub directory_ttl_ms: u64,
    pub chapter_ttl_ms: u64,
    pub register_key_ttl_ms: u64,
    pub register_key_max_entries: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SearchConfig {
    pub phase1_delay_min_ms: u64,
    pub phase1_delay_max_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct DeviceProfile {
    pub name: String,
    pub user_agent: String,
    pub cookie: String,
    pub device: UpstreamDevice,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
            sidecar: SidecarConfig::default(),
            cache: CacheConfig::default(),
            search: SearchConfig::default(),
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

impl Default for SidecarConfig {
    fn default() -> Self {
        Self {
            command: vec![
                "java".to_string(),
                "--enable-native-access=ALL-UNNAMED".to_string(),
                "-jar".to_string(),
                "/app/fq-sidecar.jar".to_string(),
            ],
            restart_cooldown_ms: 2_000,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            search_ttl_ms: 45_000,
            directory_ttl_ms: 600_000,
            chapter_ttl_ms: 600_000,
            register_key_ttl_ms: 3_600_000,
            register_key_max_entries: 128,
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
        set_command(&mut self.fq.sidecar.command, "FQRS_SIDECAR_COMMAND");
        set_u64(
            &mut self.fq.sidecar.restart_cooldown_ms,
            "FQRS_SIDECAR_RESTART_COOLDOWN_MS",
        );
        set_u64(
            &mut self.fq.cache.register_key_ttl_ms,
            "FQRS_REGISTER_KEY_CACHE_TTL_MS",
        );
        set_u64(
            &mut self.fq.cache.register_key_max_entries,
            "FQRS_REGISTER_KEY_CACHE_MAX_ENTRIES",
        );
    }

    fn validate(&self) -> Result<()> {
        if self.server.port == 0 {
            return Err(anyhow!("server.port 不能为空"));
        }
        if self.fq.sidecar.command.is_empty() {
            return Err(anyhow!("fq.sidecar.command 不能为空"));
        }
        if self.fq.device_profile.device.device_id.trim().is_empty() {
            return Err(anyhow!("fq.device_profile.device.device_id 不能为空"));
        }
        Ok(())
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

fn load_from_disk() -> Result<AppConfig> {
    const PATHS: [&str; 3] = [
        "configs/api.yaml",
        "configs/api.yml",
        "configs/api.example.yaml",
    ];

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
