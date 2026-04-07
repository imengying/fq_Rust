use crate::config::DeviceProfile;
use crate::models::{ServiceError, ServiceResult};
use chrono::Utc;
use indexmap::IndexMap;
use rand::Rng;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use url::Url;

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn build_url(base_url: &str, path: &str, params: &[(String, String)]) -> ServiceResult<String> {
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
    let mut headers = IndexMap::new();
    headers.insert(
        "accept".to_string(),
        "application/json; charset=utf-8,application/x-protobuf".to_string(),
    );
    headers.insert(
        "cookie".to_string(),
        normalize_install_id(&device.cookie, &device.device.install_id),
    );
    headers.insert("user-agent".to_string(), device.user_agent.clone());
    headers.insert("accept-encoding".to_string(), "gzip".to_string());
    headers.insert("x-xs-from-web".to_string(), "0".to_string());
    headers.insert(
        "x-vc-bdturing-sdk-version".to_string(),
        "3.7.2.cn".to_string(),
    );
    headers.insert(
        "x-reading-request".to_string(),
        format!("{}-{}", now, rand::thread_rng().gen_range(1..2_000_000_000u32)),
    );
    headers.insert("sdk-version".to_string(), "2".to_string());
    headers.insert("x-tt-store-region-src".to_string(), "did".to_string());
    headers.insert("x-tt-store-region".to_string(), "cn-zj".to_string());
    headers.insert("lc".to_string(), "101".to_string());
    headers.insert("x-ss-req-ticket".to_string(), now.to_string());
    headers.insert("passport-sdk-version".to_string(), "50564".to_string());
    headers.insert("x-ss-dp".to_string(), device.device.aid.clone());
    headers
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
        ("version_code".to_string(), device.device.version_code.clone()),
        ("version_name".to_string(), device.device.version_name.clone()),
        ("device_platform".to_string(), "android".to_string()),
        ("os".to_string(), "android".to_string()),
        ("ssmix".to_string(), "a".to_string()),
        ("device_type".to_string(), device.device.device_type.clone()),
        ("device_brand".to_string(), device.device.device_brand.clone()),
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
}
