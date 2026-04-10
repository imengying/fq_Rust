use crate::config::DeviceProfile;
use crate::models::{ServiceError, ServiceResult};
use chrono::Utc;
use indexmap::IndexMap;
use rand::RngExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

/// Parameter keys whose values should be percent-encoded.
/// Must match Java version's ENCODE_WHITELIST exactly.
const ENCODE_WHITELIST: &[&str] = &[
    "query",
    "client_ab_info",
    "search_source_id",
    "search_id",
    "device_type",
    "resolution",
    "rom_version",
];

pub fn build_url(base_url: &str, path: &str, params: &[(String, String)]) -> ServiceResult<String> {
    let mut url = format!("{}{}", base_url.trim_end_matches('/'), path);
    if !params.is_empty() {
        url.push('?');
        for (i, (key, value)) in params.iter().enumerate() {
            if i > 0 {
                url.push('&');
            }
            url.push_str(key);
            url.push('=');
            if ENCODE_WHITELIST.contains(&key.as_str()) {
                url.push_str(&encode_if_needed(value));
            } else {
                url.push_str(value);
            }
        }
    }
    Ok(url)
}

/// Encode value if not already encoded. Exactly matches Java's encodeIfNeeded.
///
/// Java's logic:
/// 1. Try URLDecoder.decode(value) — treats '+' as space, '%XX' as hex
/// 2. If decoded != value → value is "already encoded", return as-is
/// 3. Otherwise → URLEncoder.encode(value) — keeps [a-zA-Z0-9.*_-], space→'+', rest→%XX
fn encode_if_needed(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    // Step 1: Java URLDecoder.decode — '+' maps to space, '%XX' to byte
    let decoded = java_url_decode(value);
    if decoded != value {
        // Already encoded (decoding changed it), return as-is
        return value.to_string();
    }
    // Step 2: Java URLEncoder.encode
    java_url_encode(value)
}

/// Matches `java.net.URLDecoder.decode(value, UTF_8)`.
/// Replaces '+' with space and decodes '%XX' sequences.
fn java_url_decode(value: &str) -> String {
    let mut result = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                result.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                    result.push(hi << 4 | lo);
                    i += 3;
                } else {
                    result.push(b'%');
                    i += 1;
                }
            }
            ch => {
                result.push(ch);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&result).into_owned()
}

/// Matches `java.net.URLEncoder.encode(value, UTF_8)`.
/// Keeps: letters, digits, '.', '-', '*', '_'
/// Space → '+'
/// Everything else → '%XX' (per byte of UTF-8 encoding)
fn java_url_encode(value: &str) -> String {
    let mut result = String::with_capacity(value.len() * 2);
    for byte in value.as_bytes() {
        match *byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'-' | b'*' | b'_' => {
                result.push(*byte as char);
            }
            b' ' => result.push('+'),
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn normalize_install_id(cookie: &str, install_id: &str) -> String {
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

pub fn build_common_headers(device: &DeviceProfile) -> IndexMap<String, String> {
    let now = now_ms();
    let normalized_cookie = normalize_install_id(&device.cookie, &device.device.install_id);
    let mut headers = IndexMap::new();
    headers.insert(
        "accept".to_string(),
        "application/json; charset=utf-8,application/x-protobuf".to_string(),
    );
    headers.insert("cookie".to_string(), normalized_cookie.clone());
    headers.insert("user-agent".to_string(), device.user_agent.clone());
    headers.insert("accept-encoding".to_string(), "gzip".to_string());
    headers.insert("x-xs-from-web".to_string(), "0".to_string());
    headers.insert(
        "x-vc-bdturing-sdk-version".to_string(),
        "3.7.2.cn".to_string(),
    );
    headers.insert(
        "x-reading-request".to_string(),
        format!("{}-{}", now, rand::rng().random_range(1..2_000_000_000u32)),
    );
    headers.insert("sdk-version".to_string(), "2".to_string());
    if let Some(store_region_src) = cookie_value(&normalized_cookie, "store-region-src") {
        headers.insert("x-tt-store-region-src".to_string(), store_region_src);
    }
    if let Some(store_region) = cookie_value(&normalized_cookie, "store-region") {
        headers.insert("x-tt-store-region".to_string(), store_region);
    }
    headers.insert("lc".to_string(), "101".to_string());
    headers.insert("x-ss-req-ticket".to_string(), now.to_string());
    headers.insert("passport-sdk-version".to_string(), "50564".to_string());
    headers.insert("x-ss-dp".to_string(), device.device.aid.clone());
    headers
}

fn cookie_value(cookie: &str, key: &str) -> Option<String> {
    for pair in cookie.split(';') {
        let trimmed = pair.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (pair_key, pair_value) = trimmed.split_once('=')?;
        if pair_key.trim().eq_ignore_ascii_case(key) {
            let value = pair_value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

pub fn build_common_params(device: &DeviceProfile) -> Vec<(String, String)> {
    let now = now_ms();
    vec![
        ("iid".to_string(), device.device.install_id.clone()),
        ("device_id".to_string(), device.device.device_id.clone()),
        ("ac".to_string(), "wifi".to_string()),
        ("channel".to_string(), "googleplay".to_string()),
        ("aid".to_string(), device.device.aid.clone()),
        ("app_name".to_string(), "novelapp".to_string()),
        (
            "version_code".to_string(),
            device.device.version_code.clone(),
        ),
        (
            "version_name".to_string(),
            device.device.version_name.clone(),
        ),
        ("device_platform".to_string(), "android".to_string()),
        ("os".to_string(), "android".to_string()),
        ("ssmix".to_string(), "a".to_string()),
        ("device_type".to_string(), device.device.device_type.clone()),
        (
            "device_brand".to_string(),
            device.device.device_brand.clone(),
        ),
        ("language".to_string(), "zh".to_string()),
        ("os_api".to_string(), device.device.os_api.clone()),
        ("os_version".to_string(), device.device.os_version.clone()),
        (
            "manifest_version_code".to_string(),
            device.device.version_code.clone(),
        ),
        ("resolution".to_string(), device.device.resolution.clone()),
        ("dpi".to_string(), device.device.dpi.clone()),
        (
            "update_version_code".to_string(),
            device.device.update_version_code.clone(),
        ),
        ("_rticket".to_string(), now.to_string()),
        ("host_abi".to_string(), device.device.host_abi.clone()),
        ("dragon_device_type".to_string(), "phone".to_string()),
        ("pv_player".to_string(), device.device.version_code.clone()),
        ("compliance_status".to_string(), "0".to_string()),
        ("need_personal_recommend".to_string(), "1".to_string()),
        ("player_so_load".to_string(), "1".to_string()),
        ("is_android_pad_screen".to_string(), "0".to_string()),
        ("rom_version".to_string(), device.device.rom_version.clone()),
        ("cdid".to_string(), device.device.cdid.clone()),
    ]
}

pub fn merge_headers(
    original: &IndexMap<String, String>,
    signed: &IndexMap<String, String>,
) -> ServiceResult<HeaderMap> {
    let mut headers = HeaderMap::new();
    for (key, value) in original.iter().chain(signed.iter()) {
        let header_name = HeaderName::from_bytes(key.as_bytes())
            .map_err(|error| ServiceError::internal(format!("非法请求头名称 {key}: {error}")))?;
        let header_value = HeaderValue::from_str(value)
            .map_err(|error| ServiceError::internal(format!("非法请求头值 {key}: {error}")))?;
        headers.insert(header_name, header_value);
    }
    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_install_id() {
        let cookie = "store-region=cn-zj; store-region-src=did";
        assert_eq!(
            normalize_install_id(cookie, "123"),
            "store-region=cn-zj; store-region-src=did; install_id=123;"
        );
    }

    #[test]
    fn extracts_cookie_values_case_insensitively() {
        let cookie = "store-region=us; STORE-REGION-SRC=did; install_id=123";
        assert_eq!(cookie_value(cookie, "store-region").as_deref(), Some("us"));
        assert_eq!(
            cookie_value(cookie, "x").as_deref(),
            None
        );
        assert_eq!(
            cookie_value(cookie, "store-region-src").as_deref(),
            Some("did")
        );
    }

    #[test]
    fn build_common_headers_uses_cookie_region_headers() {
        let mut device = DeviceProfile::default();
        device.cookie = "store-region=us; store-region-src=did; install_id=1".to_string();

        let headers = build_common_headers(&device);

        assert_eq!(headers.get("x-tt-store-region").map(String::as_str), Some("us"));
        assert_eq!(
            headers.get("x-tt-store-region-src").map(String::as_str),
            Some("did")
        );
    }
}
