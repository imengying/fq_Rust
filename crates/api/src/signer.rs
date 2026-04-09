use crate::config::SignerConfig;
use crate::fq::now_ms;
use crate::models::{ServiceError, ServiceResult};
use fq_signer_native::{NativeSigner, NativeSignerConfig};
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::mpsc;
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio::task;
use tracing::{info, warn};

#[derive(Clone)]
pub struct SignerClient {
    service: Arc<NativeSignerService>,
}

#[derive(Debug, Clone)]
pub struct SignResult {
    pub headers: IndexMap<String, String>,
}

struct NativeSignerService {
    tx: UnboundedSender<SignerCommand>,
}

enum SignerCommand {
    Sign {
        url: String,
        headers_text: String,
        reply: mpsc::Sender<ServiceResult<String>>,
    },
    Restart {
        reason: String,
        reply: mpsc::Sender<ServiceResult<bool>>,
    },
}

struct SignerThreadState {
    runtime: NativeSignerConfig,
    signer: Option<NativeSigner>,
    restart_cooldown_ms: u64,
    last_restart_at_ms: i64,
}

impl SignerClient {
    pub fn new(config: SignerConfig) -> ServiceResult<Self> {
        Ok(Self {
            service: Arc::new(NativeSignerService::start(
                config.restart_cooldown_ms,
                config.android_sdk_api,
            )?),
        })
    }

    pub async fn sign(
        &self,
        url: &str,
        headers: &IndexMap<String, String>,
    ) -> ServiceResult<SignResult> {
        let service = self.service.clone();
        let url = url.to_string();
        let headers = headers.clone();
        task::spawn_blocking(move || service.sign_blocking(&url, &headers))
            .await
            .map_err(|error| ServiceError::internal(format!("signer 请求执行失败: {error}")))?
    }

    pub async fn restart(&self, reason: &str) -> ServiceResult<bool> {
        let service = self.service.clone();
        let reason = reason.to_string();
        task::spawn_blocking(move || service.restart_blocking(&reason))
            .await
            .map_err(|error| ServiceError::internal(format!("signer 重启执行失败: {error}")))?
    }
}

impl NativeSignerService {
    fn start(restart_cooldown_ms: u64, android_sdk_api: u32) -> ServiceResult<Self> {
        let runtime = NativeSignerConfig::from_env(android_sdk_api)
            .map_err(|error| ServiceError::internal(format!("signer 运行时初始化失败: {error}")))?;
        let (tx, mut rx) = unbounded_channel();
        std::thread::Builder::new()
            .name("fq-native-signer".to_string())
            .spawn(move || {
                let mut state = SignerThreadState::new(runtime, restart_cooldown_ms);
                while let Some(command) = rx.blocking_recv() {
                    match command {
                        SignerCommand::Sign {
                            url,
                            headers_text,
                            reply,
                        } => {
                            let _ = reply.send(state.sign(&url, &headers_text));
                        }
                        SignerCommand::Restart { reason, reply } => {
                            let _ = reply.send(state.restart_if_allowed(&reason));
                        }
                    }
                }
            })
            .map_err(|error| ServiceError::internal(format!("signer 线程启动失败: {error}")))?;

        Ok(Self { tx })
    }

    fn sign_blocking(
        &self,
        url: &str,
        headers: &IndexMap<String, String>,
    ) -> ServiceResult<SignResult> {
        let headers_text = build_signature_input_headers(headers);
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(SignerCommand::Sign {
                url: url.to_string(),
                headers_text,
                reply: reply_tx,
            })
            .map_err(|_| ServiceError::internal("signer 线程不可用"))?;

        let raw = reply_rx
            .recv()
            .map_err(|_| ServiceError::internal("signer 响应通道已关闭"))??;
        info!(
            "signer raw output: len={}, {}",
            raw.len(),
            truncate(&raw, 800)
        );
        Ok(SignResult {
            headers: parse_signature_result(&raw)?,
        })
    }

    fn restart_blocking(&self, reason: &str) -> ServiceResult<bool> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(SignerCommand::Restart {
                reason: reason.to_string(),
                reply: reply_tx,
            })
            .map_err(|_| ServiceError::internal("signer 线程不可用"))?;
        reply_rx
            .recv()
            .map_err(|_| ServiceError::internal("signer 重启通道已关闭"))?
    }
}

impl SignerThreadState {
    fn new(runtime: NativeSignerConfig, restart_cooldown_ms: u64) -> Self {
        Self {
            runtime,
            signer: None,
            restart_cooldown_ms,
            last_restart_at_ms: 0,
        }
    }

    fn sign(&mut self, url: &str, headers_text: &str) -> ServiceResult<String> {
        self.ensure_started()?;
        match self
            .signer
            .as_mut()
            .expect("signer initialized")
            .sign(url, headers_text)
        {
            Ok(raw) => Ok(raw),
            Err(error) => {
                let initial_error = ServiceError::internal(format!("signer 请求失败: {error}"));
                if self.should_restart_after_sign_error(&initial_error)
                    && self.restart_if_allowed("AUTO_RESTART:SIGNER_ERROR")?
                {
                    self.ensure_started()?;
                    return self
                        .signer
                        .as_mut()
                        .expect("signer restarted")
                        .sign(url, headers_text)
                        .map_err(|retry_error| {
                            ServiceError::internal(format!("signer 请求失败: {retry_error}"))
                        });
                }
                Err(initial_error)
            }
        }
    }

    fn ensure_started(&mut self) -> ServiceResult<()> {
        if self.signer.is_none() {
            self.signer = Some(self.create_signer()?);
        }
        Ok(())
    }

    fn restart_if_allowed(&mut self, reason: &str) -> ServiceResult<bool> {
        let now = now_ms();
        if self.restart_cooldown_ms > 0
            && self.last_restart_at_ms > 0
            && now - self.last_restart_at_ms < self.restart_cooldown_ms as i64
        {
            return Ok(false);
        }

        warn!("restarting native signer: reason={reason}");
        self.signer = Some(self.create_signer()?);
        self.last_restart_at_ms = now;
        Ok(true)
    }

    fn should_restart_after_sign_error(&self, error: &ServiceError) -> bool {
        error.code == 1003 || error.code == -1
    }

    fn create_signer(&self) -> ServiceResult<NativeSigner> {
        NativeSigner::new(self.runtime.clone())
            .map_err(|error| ServiceError::internal(format!("signer 初始化失败: {error}")))
    }
}

fn truncate(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        value.to_string()
    } else {
        let mut end = max_len;
        while end > 0 && !value.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &value[..end])
    }
}

fn build_signature_input_headers(headers: &IndexMap<String, String>) -> String {
    let mut builder = String::new();
    let mut first = true;
    for (key, value) in headers {
        if !first {
            builder.push_str("\r\n");
        }
        builder.push_str(key);
        builder.push_str("\r\n");
        builder.push_str(value);
        first = false;
    }
    builder
}

fn parse_signature_result(raw: &str) -> ServiceResult<IndexMap<String, String>> {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Err(ServiceError::internal("signer 返回空签名结果"));
    }

    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        let parsed: IndexMap<String, String> = serde_json::from_str(trimmed).map_err(|error| {
            ServiceError::internal(format!("signer 签名 JSON 解析失败: {error}"))
        })?;
        return Ok(remove_header_ignore_case(parsed, "X-Neptune"));
    }

    let lines: Vec<&str> = trimmed.split('\n').collect();
    let mut result = IndexMap::new();

    if looks_like_colon_pairs(&lines) {
        for line in lines {
            let value = line.trim();
            if value.is_empty() {
                continue;
            }
            if let Some((key, raw_value)) = value.split_once(':') {
                put_header(&mut result, key, raw_value);
            }
        }
    } else if lines.len() >= 2 && lines.len() % 2 == 0 {
        for pair in lines.chunks(2) {
            if let [key, value] = pair {
                put_header(&mut result, key, value);
            }
        }
    } else {
        for line in lines {
            let value = line.trim();
            if value.is_empty() {
                continue;
            }
            if let Some((key, raw_value)) = value.split_once('=') {
                put_header(&mut result, key, raw_value);
            }
        }
    }

    Ok(remove_header_ignore_case(result, "X-Neptune"))
}

static HEADER_COLON_PAIR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[A-Za-z0-9-]{1,64}:\s*.+$").unwrap());

fn looks_like_colon_pairs(lines: &[&str]) -> bool {
    lines
        .iter()
        .any(|line| HEADER_COLON_PAIR.is_match(line.trim()))
}

fn put_header(result: &mut IndexMap<String, String>, raw_key: &str, raw_value: &str) {
    let key = raw_key.trim();
    if key.is_empty() {
        return;
    }
    result.insert(key.to_string(), raw_value.trim().to_string());
}

fn remove_header_ignore_case(
    headers: IndexMap<String, String>,
    target: &str,
) -> IndexMap<String, String> {
    headers
        .into_iter()
        .filter(|(key, _)| !key.eq_ignore_ascii_case(target))
        .collect()
}
