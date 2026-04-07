use crate::config::DeviceProfile;
use crate::models::{ServiceError, ServiceResult};
use indexmap::IndexMap;
use reqwest::StatusCode;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Clone)]
pub struct SidecarClient {
    client: reqwest::Client,
    base_url: String,
    internal_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignResult {
    pub headers: IndexMap<String, String>,
    pub signer_epoch: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterKeyResolveResult {
    pub device_fingerprint: String,
    pub keyver: i64,
    pub real_key_hex: String,
    pub expires_at_ms: i64,
    pub source: String,
}

impl SidecarClient {
    pub fn new(client: reqwest::Client, base_url: impl Into<String>, internal_token: impl Into<String>) -> Self {
        Self {
            client,
            base_url: base_url.into(),
            internal_token: internal_token.into(),
        }
    }

    pub async fn sign(&self, url: &str, headers: &IndexMap<String, String>) -> ServiceResult<SignResult> {
        let request = SignRequest { url, headers };
        self.post_json("/internal/v1/sign", &request).await
    }

    pub async fn resolve_register_key(
        &self,
        device_profile: &DeviceProfile,
        required_keyver: Option<i64>,
    ) -> ServiceResult<RegisterKeyResolveResult> {
        let request = RegisterKeyResolveRequest {
            device_profile,
            required_keyver,
        };
        self.post_json("/internal/v1/register-key/resolve", &request).await
    }

    pub async fn invalidate_register_key(&self, device_fingerprint: &str) -> ServiceResult<()> {
        let request = RegisterKeyInvalidateRequest {
            device_fingerprint,
        };
        let _: RegisterKeyInvalidateResult =
            self.post_json("/internal/v1/register-key/invalidate", &request).await?;
        Ok(())
    }

    async fn post_json<B, T>(&self, path: &str, body: &B) -> ServiceResult<T>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        let response = self
            .client
            .post(url)
            .header("X-Internal-Token", &self.internal_token)
            .json(body)
            .send()
            .await
            .map_err(|error| ServiceError::internal(format!("sidecar 请求失败: {error}")))?;

        parse_envelope(response).await
    }
}

#[derive(Serialize)]
struct SignRequest<'a> {
    url: &'a str,
    headers: &'a IndexMap<String, String>,
}

#[derive(Serialize)]
struct RegisterKeyResolveRequest<'a> {
    device_profile: &'a DeviceProfile,
    #[serde(skip_serializing_if = "Option::is_none")]
    required_keyver: Option<i64>,
}

#[derive(Serialize)]
struct RegisterKeyInvalidateRequest<'a> {
    device_fingerprint: &'a str,
}

#[derive(Deserialize)]
struct SidecarEnvelope<T> {
    code: i32,
    message: String,
    data: Option<T>,
}

#[derive(Deserialize)]
struct RegisterKeyInvalidateResult {
    invalidated: bool,
}

async fn parse_envelope<T>(response: reqwest::Response) -> ServiceResult<T>
where
    T: DeserializeOwned,
{
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| ServiceError::internal(format!("sidecar 响应读取失败: {error}")))?;

    let envelope: SidecarEnvelope<T> = serde_json::from_str(&text).map_err(|error| {
        ServiceError::internal(format!(
            "sidecar 响应解析失败: {error}; raw={}",
            truncate(&text, 512)
        ))
    })?;

    if status != StatusCode::OK {
        return Err(ServiceError::new(envelope.code, envelope.message));
    }

    if envelope.code != 0 {
        return Err(ServiceError::new(envelope.code, envelope.message));
    }

    envelope
        .data
        .ok_or_else(|| ServiceError::internal("sidecar 返回空 data"))
}

fn truncate(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        value.to_string()
    } else {
        format!("{}...", &value[..max_len])
    }
}

