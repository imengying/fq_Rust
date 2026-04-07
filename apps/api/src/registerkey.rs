use crate::config::{DeviceProfile, UpstreamConfig};
use crate::models::{ServiceError, ServiceResult};
use crate::sidecar::SidecarClient;
use aes::Aes128;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use chrono::Utc;
use dashmap::DashMap;
use flate2::read::GzDecoder;
use indexmap::IndexMap;
use rand::Rng;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::sync::Arc;
use url::Url;

type Aes128CbcDec = cbc::Decryptor<Aes128>;
type Aes128CbcEnc = cbc::Encryptor<Aes128>;

const REGISTER_KEY_PATH: &str = "/reading/crypt/registerkey";
const REGISTER_KEY_FIXED_AES_HEX: &str = "ac25c67ddd8f38c1b37a2348828e222e";

#[derive(Clone)]
pub struct RegisterKeyService {
    cache_by_key: Arc<DashMap<String, CacheEntry>>,
    current_by_fingerprint: Arc<DashMap<String, String>>,
    cache_ttl_ms: u64,
    cache_max_entries: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterKeyResolveResult {
    pub device_fingerprint: String,
    pub keyver: i64,
    pub real_key_hex: String,
    pub expires_at_ms: i64,
    pub source: String,
}

impl RegisterKeyResolveResult {
    fn with_source(&self, value: &str) -> Self {
        let mut cloned = self.clone();
        cloned.source = value.to_string();
        cloned
    }
}

#[derive(Debug, Clone)]
struct CacheEntry {
    result: RegisterKeyResolveResult,
    expires_at_ms: i64,
}

impl RegisterKeyService {
    pub fn new(cache_ttl_ms: u64, cache_max_entries: usize) -> Self {
        Self {
            cache_by_key: Arc::new(DashMap::new()),
            current_by_fingerprint: Arc::new(DashMap::new()),
            cache_ttl_ms,
            cache_max_entries: cache_max_entries.max(1),
        }
    }

    pub async fn resolve(
        &self,
        http_client: &reqwest::Client,
        sidecar_client: &SidecarClient,
        upstream: &UpstreamConfig,
        profile: &DeviceProfile,
        required_keyver: Option<i64>,
    ) -> ServiceResult<RegisterKeyResolveResult> {
        let fingerprint = fingerprint(profile);
        let normalized_keyver = required_keyver.filter(|value| *value > 0);

        if let Some(keyver) = normalized_keyver {
            if let Some(cached) = self.get_valid(&cache_key(&fingerprint, keyver)) {
                return Ok(cached.with_source("cache"));
            }
        } else if let Some(cache_key) = self.current_cache_key(&fingerprint) {
            if let Some(cached) = self.get_valid(&cache_key) {
                return Ok(cached.with_source("cache"));
            }
            self.current_by_fingerprint.remove(&fingerprint);
        }

        let fetched = fetch_register_key(http_client, sidecar_client, upstream, profile).await?;
        if let Some(keyver) = normalized_keyver {
            if fetched.keyver != keyver {
                return Err(ServiceError::new(1101, "registerkey version mismatch"));
            }
        }

        let expires_at_ms = compute_expires_at_ms(self.cache_ttl_ms);
        let result = RegisterKeyResolveResult {
            device_fingerprint: fingerprint.clone(),
            keyver: fetched.keyver,
            real_key_hex: fetched.real_key_hex,
            expires_at_ms,
            source: "refresh".to_string(),
        };
        let key = cache_key(&fingerprint, fetched.keyver);
        self.cache_by_key.insert(
            key.clone(),
            CacheEntry {
                result: result.clone(),
                expires_at_ms,
            },
        );
        self.current_by_fingerprint.insert(fingerprint, key);
        self.trim_if_needed();
        Ok(result)
    }

    pub fn invalidate(&self, device_fingerprint: &str) -> ServiceResult<bool> {
        if device_fingerprint.trim().is_empty() {
            return Err(ServiceError::bad_request("device_fingerprint 不能为空"));
        }

        let mut removed = self.current_by_fingerprint.remove(device_fingerprint).is_some();
        let prefix = format!("{device_fingerprint}:");
        let keys: Vec<String> = self
            .cache_by_key
            .iter()
            .filter_map(|entry| {
                if entry.key().starts_with(&prefix) {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect();
        for key in keys {
            if self.cache_by_key.remove(&key).is_some() {
                removed = true;
            }
        }
        Ok(removed)
    }

    fn current_cache_key(&self, fingerprint: &str) -> Option<String> {
        self.current_by_fingerprint
            .get(fingerprint)
            .map(|value| value.value().clone())
    }

    fn get_valid(&self, key: &str) -> Option<RegisterKeyResolveResult> {
        let entry = self.cache_by_key.get(key)?;
        if entry.expires_at_ms < now_ms() {
            drop(entry);
            self.cache_by_key.remove(key);
            return None;
        }
        Some(entry.result.clone())
    }

    fn trim_if_needed(&self) {
        let overflow = self.cache_by_key.len().saturating_sub(self.cache_max_entries);
        if overflow == 0 {
            return;
        }

        let keys_to_remove: Vec<String> = self
            .cache_by_key
            .iter()
            .take(overflow)
            .map(|entry| entry.key().clone())
            .collect();
        for key in keys_to_remove {
            if self.cache_by_key.remove(&key).is_some() {
                let fingerprints: Vec<String> = self
                    .current_by_fingerprint
                    .iter()
                    .filter_map(|entry| {
                        if entry.value() == &key {
                            Some(entry.key().clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                for fingerprint in fingerprints {
                    self.current_by_fingerprint.remove(&fingerprint);
                }
            }
        }
    }
}

async fn fetch_register_key(
    http_client: &reqwest::Client,
    sidecar_client: &SidecarClient,
    upstream: &UpstreamConfig,
    profile: &DeviceProfile,
) -> ServiceResult<FetchedRegisterKey> {
    let current_time = now_ms();
    let url = build_register_key_url(upstream, profile, current_time)?;
    let headers = build_register_key_headers(profile, current_time);
    let signed = sidecar_client.sign(&url, &headers).await?;

    let payload = FqRegisterKeyPayload {
        content: new_register_key_content(&profile.device.device_id, "0")?,
        keyver: 1,
    };
    let response = http_client
        .post(&url)
        .headers(merge_headers(&headers, &signed.headers)?)
        .json(&payload)
        .send()
        .await
        .map_err(|error| ServiceError::internal(format!("registerkey upstream 请求失败: {error}")))?;

    let status = response.status();
    let body = response
        .bytes()
        .await
        .map_err(|error| ServiceError::internal(format!("registerkey upstream 响应读取失败: {error}")))?;
    let response_body = decode_gzip_or_utf8(body.as_ref())?;

    if response_body.trim().is_empty() {
        return Err(ServiceError::internal("registerkey upstream 返回空响应"));
    }
    if !status.is_success() {
        return Err(ServiceError::internal(format!(
            "registerkey upstream HTTP状态异常: {}",
            status.as_u16()
        )));
    }

    let parsed: FqRegisterKeyResponse = serde_json::from_str(&response_body)
        .map_err(|error| ServiceError::internal(format!("registerkey upstream JSON 解析失败: {error}")))?;
    if parsed.code != 0 {
        return Err(ServiceError::internal(format!(
            "registerkey upstream 失败: {}",
            parsed.message
        )));
    }
    let payload = parsed
        .data
        .ok_or_else(|| ServiceError::internal("registerkey upstream 返回无效数据"))?;
    if payload.key.trim().is_empty() {
        return Err(ServiceError::internal("registerkey upstream 返回无效数据"));
    }

    Ok(FetchedRegisterKey {
        keyver: payload.keyver,
        real_key_hex: extract_real_key(&payload.key)?,
    })
}

fn build_register_key_url(
    upstream: &UpstreamConfig,
    profile: &DeviceProfile,
    current_time: i64,
) -> ServiceResult<String> {
    let params = vec![
        ("iid".to_string(), profile.device.install_id.clone()),
        ("device_id".to_string(), profile.device.device_id.clone()),
        ("ac".to_string(), "wifi".to_string()),
        ("channel".to_string(), "googleplay".to_string()),
        ("aid".to_string(), profile.device.aid.clone()),
        ("app_name".to_string(), "novelapp".to_string()),
        ("version_code".to_string(), profile.device.version_code.clone()),
        ("version_name".to_string(), profile.device.version_name.clone()),
        ("device_platform".to_string(), "android".to_string()),
        ("os".to_string(), "android".to_string()),
        ("ssmix".to_string(), "a".to_string()),
        ("device_type".to_string(), profile.device.device_type.clone()),
        ("device_brand".to_string(), profile.device.device_brand.clone()),
        ("language".to_string(), "zh".to_string()),
        ("os_api".to_string(), profile.device.os_api.clone()),
        ("os_version".to_string(), profile.device.os_version.clone()),
        (
            "manifest_version_code".to_string(),
            profile.device.version_code.clone(),
        ),
        ("resolution".to_string(), profile.device.resolution.clone()),
        ("dpi".to_string(), profile.device.dpi.clone()),
        (
            "update_version_code".to_string(),
            profile.device.update_version_code.clone(),
        ),
        ("_rticket".to_string(), current_time.to_string()),
        ("host_abi".to_string(), profile.device.host_abi.clone()),
        ("dragon_device_type".to_string(), "phone".to_string()),
        ("pv_player".to_string(), profile.device.version_code.clone()),
        ("compliance_status".to_string(), "0".to_string()),
        ("need_personal_recommend".to_string(), "1".to_string()),
        ("player_so_load".to_string(), "1".to_string()),
        ("is_android_pad_screen".to_string(), "0".to_string()),
        ("rom_version".to_string(), profile.device.rom_version.clone()),
        ("cdid".to_string(), profile.device.cdid.clone()),
    ];
    build_url(&upstream.base_url, REGISTER_KEY_PATH, &params)
}

fn build_register_key_headers(
    profile: &DeviceProfile,
    current_time: i64,
) -> IndexMap<String, String> {
    let mut headers = IndexMap::new();
    headers.insert(
        "accept".to_string(),
        "application/json; charset=utf-8,application/x-protobuf".to_string(),
    );
    headers.insert(
        "cookie".to_string(),
        normalize_install_id(&profile.cookie, &profile.device.install_id),
    );
    headers.insert("user-agent".to_string(), profile.user_agent.clone());
    headers.insert("accept-encoding".to_string(), "gzip".to_string());
    headers.insert("x-xs-from-web".to_string(), "0".to_string());
    headers.insert(
        "x-vc-bdturing-sdk-version".to_string(),
        "3.7.2.cn".to_string(),
    );
    headers.insert(
        "x-reading-request".to_string(),
        format!(
            "{}-{}",
            current_time,
            rand::thread_rng().gen_range(1..2_000_000_000u32)
        ),
    );
    headers.insert("sdk-version".to_string(), "2".to_string());
    headers.insert("x-tt-store-region-src".to_string(), "did".to_string());
    headers.insert("x-tt-store-region".to_string(), "cn-zj".to_string());
    headers.insert("lc".to_string(), "101".to_string());
    headers.insert("x-ss-req-ticket".to_string(), current_time.to_string());
    headers.insert("passport-sdk-version".to_string(), "50564".to_string());
    headers.insert("x-ss-dp".to_string(), profile.device.aid.clone());
    headers.insert("content-type".to_string(), "application/json".to_string());
    headers
}

fn merge_headers(
    original: &IndexMap<String, String>,
    signed: &IndexMap<String, String>,
) -> ServiceResult<HeaderMap> {
    let mut headers = HeaderMap::new();
    for (key, value) in original.iter().chain(signed.iter()) {
        let header_name = HeaderName::from_bytes(key.as_bytes()).map_err(|error| {
            ServiceError::internal(format!("非法请求头名称 {key}: {error}"))
        })?;
        let header_value = HeaderValue::from_str(value).map_err(|error| {
            ServiceError::internal(format!("非法请求头值 {key}: {error}"))
        })?;
        headers.insert(header_name, header_value);
    }
    Ok(headers)
}

fn build_url(base_url: &str, path: &str, params: &[(String, String)]) -> ServiceResult<String> {
    let raw = format!("{}{}", base_url.trim_end_matches('/'), path);
    let mut url = Url::parse(&raw)
        .map_err(|error| ServiceError::internal(format!("URL 构建失败 {raw}: {error}")))?;
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in params {
            pairs.append_pair(key, value);
        }
    }
    Ok(url.to_string())
}

fn normalize_install_id(cookie: &str, install_id: &str) -> String {
    if install_id.trim().is_empty() {
        return cookie.to_string();
    }
    let key = "install_id=";
    if let Some(position) = cookie.to_ascii_lowercase().find(key) {
        let value_start = position + key.len();
        let value_end = cookie[value_start..]
            .find(';')
            .map(|offset| value_start + offset)
            .unwrap_or(cookie.len());
        let mut output = String::new();
        output.push_str(&cookie[..value_start]);
        output.push_str(install_id);
        output.push_str(&cookie[value_end..]);
        output
    } else if cookie.trim().is_empty() {
        format!("{key}{install_id}")
    } else if cookie.trim().ends_with(';') {
        format!("{} {key}{install_id};", cookie.trim())
    } else {
        format!("{}; {key}{install_id};", cookie.trim())
    }
}

fn new_register_key_content(server_device_id: &str, value: &str) -> ServiceResult<String> {
    let device_id = server_device_id
        .trim()
        .parse::<i64>()
        .map_err(|error| ServiceError::internal(format!("device_id 非法: {error}")))?;
    let numeric_value = value
        .trim()
        .parse::<i64>()
        .map_err(|error| ServiceError::internal(format!("registerkey 内容非法: {error}")))?;

    let mut plaintext = Vec::with_capacity(16);
    plaintext.extend_from_slice(&device_id.to_le_bytes());
    plaintext.extend_from_slice(&numeric_value.to_le_bytes());

    let key = decode_hex(REGISTER_KEY_FIXED_AES_HEX)?;
    let iv: [u8; 16] = rand::thread_rng().gen();
    let mut buffer = vec![0u8; plaintext.len() + 16];
    buffer[..plaintext.len()].copy_from_slice(&plaintext);
    let encrypted = Aes128CbcEnc::new_from_slices(&key, &iv)
        .map_err(|error| ServiceError::internal(format!("registerkey AES 初始化失败: {error}")))?
        .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
        .map_err(|error| ServiceError::internal(format!("registerkey 加密失败: {error}")))?;

    let mut payload = iv.to_vec();
    payload.extend_from_slice(encrypted);
    Ok(BASE64.encode(payload))
}

fn extract_real_key(registerkey_response_key: &str) -> ServiceResult<String> {
    let raw = BASE64
        .decode(registerkey_response_key)
        .map_err(|error| ServiceError::internal(format!("registerkey Base64 解码失败: {error}")))?;
    if raw.len() < 16 {
        return Err(ServiceError::internal("registerkey 响应过短"));
    }

    let (iv, ciphertext) = raw.split_at(16);
    let key = decode_hex(REGISTER_KEY_FIXED_AES_HEX)?;
    let mut buffer = ciphertext.to_vec();
    let decrypted = Aes128CbcDec::new_from_slices(&key, iv)
        .map_err(|error| ServiceError::internal(format!("registerkey AES 初始化失败: {error}")))?
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|error| ServiceError::internal(format!("registerkey 解密失败: {error}")))?;

    let full_key = bytes_to_upper_hex(decrypted);
    if full_key.len() < 32 {
        return Err(ServiceError::internal("registerkey 解密后的密钥长度不足"));
    }
    Ok(full_key[..32].to_string())
}

fn decode_hex(input: &str) -> ServiceResult<Vec<u8>> {
    let normalized = input.trim();
    if normalized.len() != 32 {
        return Err(ServiceError::internal("registerkey AES key 长度非法"));
    }
    let mut bytes = Vec::with_capacity(16);
    let chars: Vec<char> = normalized.chars().collect();
    for pair in chars.chunks(2) {
        let hi = pair
            .first()
            .and_then(|value| value.to_digit(16))
            .ok_or_else(|| ServiceError::internal("registerkey AES key 非法"))?;
        let lo = pair
            .get(1)
            .and_then(|value| value.to_digit(16))
            .ok_or_else(|| ServiceError::internal("registerkey AES key 非法"))?;
        bytes.push(((hi << 4) | lo) as u8);
    }
    Ok(bytes)
}

fn decode_gzip_or_utf8(raw: &[u8]) -> ServiceResult<String> {
    if raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b {
        let mut decoder = GzDecoder::new(raw);
        let mut output = String::new();
        decoder
            .read_to_string(&mut output)
            .map_err(|error| ServiceError::internal(format!("GZIP 解压失败: {error}")))?;
        return Ok(output);
    }
    Ok(String::from_utf8_lossy(raw).to_string())
}

fn fingerprint(profile: &DeviceProfile) -> String {
    let raw = [
        profile.name.as_str(),
        profile.user_agent.as_str(),
        profile.cookie.as_str(),
        profile.device.aid.as_str(),
        profile.device.cdid.as_str(),
        profile.device.device_id.as_str(),
        profile.device.device_type.as_str(),
        profile.device.device_brand.as_str(),
        profile.device.install_id.as_str(),
        profile.device.version_code.as_str(),
        profile.device.version_name.as_str(),
        profile.device.update_version_code.as_str(),
        profile.device.resolution.as_str(),
        profile.device.dpi.as_str(),
        profile.device.rom_version.as_str(),
        profile.device.host_abi.as_str(),
        profile.device.os_version.as_str(),
        profile.device.os_api.as_str(),
    ]
    .iter()
    .map(|value| value.trim())
    .collect::<Vec<_>>()
    .join("|");

    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|value| format!("{value:02x}")).collect()
}

fn bytes_to_upper_hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02X}")).collect()
}

fn cache_key(fingerprint: &str, keyver: i64) -> String {
    format!("{fingerprint}:{keyver}")
}

fn compute_expires_at_ms(ttl_ms: u64) -> i64 {
    if ttl_ms == 0 {
        i64::MAX
    } else {
        now_ms().saturating_add(ttl_ms as i64)
    }
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

struct FetchedRegisterKey {
    keyver: i64,
    real_key_hex: String,
}

#[derive(Serialize)]
struct FqRegisterKeyPayload {
    content: String,
    keyver: i64,
}

#[derive(Deserialize)]
struct FqRegisterKeyResponse {
    code: i64,
    message: String,
    data: Option<FqRegisterKeyPayloadResponse>,
}

#[derive(Deserialize)]
struct FqRegisterKeyPayloadResponse {
    key: String,
    keyver: i64,
}
