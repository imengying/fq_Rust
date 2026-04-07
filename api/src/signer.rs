use crate::config::{SignerBackendKind, SignerConfig};
use crate::fq::now_ms;
use crate::models::{ServiceError, ServiceResult};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::task;
use tracing::{info, warn};

#[derive(Clone)]
pub struct SignerClient {
    backend: Arc<dyn SignerBackend>,
}

#[derive(Debug, Clone)]
pub struct SignResult {
    pub headers: IndexMap<String, String>,
}

trait SignerBackend: Send + Sync {
    fn sign_blocking(&self, url: &str, headers: &IndexMap<String, String>) -> ServiceResult<SignResult>;
    fn restart_blocking(&self, reason: &str) -> ServiceResult<bool>;
}

impl SignerClient {
    pub fn new(config: SignerConfig) -> ServiceResult<Self> {
        let backend: Arc<dyn SignerBackend> = match config.backend {
            SignerBackendKind::JavaWorker => Arc::new(ProcessSignerBackend::new(
                "java_worker",
                config.command,
                config.restart_cooldown_ms,
            )?),
            SignerBackendKind::RustNative => Arc::new(ProcessSignerBackend::new(
                "rust_native",
                config.command,
                config.restart_cooldown_ms,
            )?),
        };
        Ok(Self { backend })
    }

    pub async fn sign(&self, url: &str, headers: &IndexMap<String, String>) -> ServiceResult<SignResult> {
        let backend = self.backend.clone();
        let url = url.to_string();
        let headers = headers.clone();
        task::spawn_blocking(move || {
            backend.sign_blocking(&url, &headers)
        })
        .await
        .map_err(|error| ServiceError::internal(format!("signer 请求执行失败: {error}")))?
    }

    pub async fn restart(&self, reason: &str) -> ServiceResult<bool> {
        let backend = self.backend.clone();
        let reason = reason.to_string();
        task::spawn_blocking(move || {
            backend.restart_blocking(&reason)
        })
        .await
        .map_err(|error| ServiceError::internal(format!("signer 重启执行失败: {error}")))?
    }
}

struct ProcessSignerBackend {
    name: &'static str,
    inner: Arc<Mutex<SignerProcess>>,
}

impl ProcessSignerBackend {
    fn new(
        name: &'static str,
        command: Vec<String>,
        restart_cooldown_ms: u64,
    ) -> ServiceResult<Self> {
        let mut process = SignerProcess::new(command, restart_cooldown_ms);
        process.ensure_started()?;
        Ok(Self {
            name,
            inner: Arc::new(Mutex::new(process)),
        })
    }
}

impl SignerBackend for ProcessSignerBackend {
    fn sign_blocking(&self, url: &str, headers: &IndexMap<String, String>) -> ServiceResult<SignResult> {
        let headers_text = build_signature_input_headers(headers);
        let mut process = self
            .inner
            .lock()
            .map_err(|_| ServiceError::internal("signer 进程锁异常"))?;

        match process.sign(url, &headers_text) {
            Ok(raw) => {
                info!(
                    "signer raw output: backend={}, len={}, {}",
                    self.name,
                    raw.len(),
                    truncate(&raw, 800)
                );
                Ok(SignResult {
                    headers: parse_signature_result(&raw)?,
                })
            }
            Err(error) => {
                if process.should_restart_after_sign_error(&error)
                    && process.restart_if_allowed("AUTO_RESTART:SIGNER_ERROR")?
                {
                    let raw = process.sign(url, &headers_text)?;
                    return Ok(SignResult {
                        headers: parse_signature_result(&raw)?,
                    });
                }
                Err(error)
            }
        }
    }

    fn restart_blocking(&self, reason: &str) -> ServiceResult<bool> {
        let mut process = self
            .inner
            .lock()
            .map_err(|_| ServiceError::internal("signer 进程锁异常"))?;
        process.restart_if_allowed(reason)
    }
}

struct SignerProcess {
    command: Vec<String>,
    restart_cooldown_ms: u64,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    last_restart_at_ms: i64,
}

impl SignerProcess {
    fn new(command: Vec<String>, restart_cooldown_ms: u64) -> Self {
        Self {
            command,
            restart_cooldown_ms,
            child: None,
            stdin: None,
            stdout: None,
            last_restart_at_ms: 0,
        }
    }

    fn sign(&mut self, url: &str, headers_text: &str) -> ServiceResult<String> {
        self.ensure_started()?;
        let payload = format!(
            "sign\t{}\t{}\n",
            encode_protocol_field(url),
            encode_protocol_field(headers_text)
        );

        let write_result = {
            let stdin = self
                .stdin
                .as_mut()
                .ok_or_else(|| ServiceError::internal("signer stdin 不可用"))?;
            stdin
                .write_all(payload.as_bytes())
                .and_then(|_| stdin.flush())
        };
        if let Err(error) = write_result {
            self.reset_process();
            return Err(ServiceError::internal(format!("signer 请求写入失败: {error}")));
        }

        let mut line = String::new();
        let read_result = {
            let stdout = self
                .stdout
                .as_mut()
                .ok_or_else(|| ServiceError::internal("signer stdout 不可用"))?;
            stdout.read_line(&mut line)
        };
        let read = match read_result {
            Ok(value) => value,
            Err(error) => {
                self.reset_process();
                return Err(ServiceError::internal(format!("signer 响应读取失败: {error}")));
            }
        };
        if read == 0 {
            self.reset_process();
            return Err(ServiceError::internal("signer 已退出"));
        }

        parse_sign_response(&line)
    }

    fn ensure_started(&mut self) -> ServiceResult<()> {
        if let Some(child) = self.child.as_mut() {
            if child
                .try_wait()
                .map_err(|error| ServiceError::internal(format!("signer 状态检查失败: {error}")))?
                .is_none()
            {
                return Ok(());
            }
            self.reset_process();
        }

        self.spawn_process()
    }

    fn spawn_process(&mut self) -> ServiceResult<()> {
        let binary = self
            .command
            .first()
            .ok_or_else(|| ServiceError::internal("fq.signer.command 不能为空"))?
            .clone();
        let mut command = Command::new(binary);
        if self.command.len() > 1 {
            command.args(&self.command[1..]);
        }
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());

        let mut child = command
            .spawn()
            .map_err(|error| ServiceError::internal(format!("signer 启动失败: {error}")))?;
        self.stdin = child.stdin.take();
        self.stdout = child.stdout.take().map(BufReader::new);
        self.child = Some(child);
        Ok(())
    }

    fn should_restart_after_sign_error(&self, error: &ServiceError) -> bool {
        error.code == 1003 || error.code == -1
    }

    fn restart_if_allowed(&mut self, reason: &str) -> ServiceResult<bool> {
        let now = now_ms();
        if self.restart_cooldown_ms > 0
            && self.last_restart_at_ms > 0
            && now - self.last_restart_at_ms < self.restart_cooldown_ms as i64
        {
            return Ok(false);
        }

        warn!("restarting signer process: reason={reason}");
        self.reset_process();
        self.spawn_process()?;
        self.last_restart_at_ms = now;
        Ok(true)
    }

    fn reset_process(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.stdin = None;
        self.stdout = None;
    }
}

impl Drop for SignerProcess {
    fn drop(&mut self) {
        self.reset_process();
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

fn encode_protocol_field(value: &str) -> String {
    URL_SAFE_NO_PAD.encode(value.as_bytes())
}

fn decode_protocol_field(field: &str, name: &str) -> ServiceResult<String> {
    let bytes = URL_SAFE_NO_PAD.decode(field).map_err(|error| {
        ServiceError::internal(format!("signer {name} 字段解码失败: {error}"))
    })?;
    String::from_utf8(bytes)
        .map_err(|error| ServiceError::internal(format!("signer {name} 字段不是合法 UTF-8: {error}")))
}

fn parse_sign_response(line: &str) -> ServiceResult<String> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed.is_empty() {
        return Err(ServiceError::internal("signer 返回空响应"));
    }

    let mut parts = trimmed.split('\t');
    let status = parts
        .next()
        .ok_or_else(|| ServiceError::internal("signer 响应缺少状态字段"))?;

    match status {
        "ok" => {
            let raw = parts
                .next()
                .ok_or_else(|| ServiceError::internal("signer 响应缺少签名字段"))?;
            if parts.next().is_some() {
                return Err(ServiceError::internal(format!(
                    "signer ok 响应字段异常: raw={}",
                    truncate(trimmed, 512)
                )));
            }
            decode_protocol_field(raw, "raw")
        }
        "err" => {
            let code = parts
                .next()
                .ok_or_else(|| ServiceError::internal("signer 错误响应缺少 code"))?;
            let message = parts
                .next()
                .ok_or_else(|| ServiceError::internal("signer 错误响应缺少 message"))?;
            if parts.next().is_some() {
                return Err(ServiceError::internal(format!(
                    "signer err 响应字段异常: raw={}",
                    truncate(trimmed, 512)
                )));
            }

            let code = code.parse::<i32>().map_err(|error| {
                ServiceError::internal(format!(
                    "signer 错误码解析失败: {error}; raw={}",
                    truncate(trimmed, 512)
                ))
            })?;
            let message = decode_protocol_field(message, "message")?;
            Err(ServiceError::new(code, message))
        }
        _ => Err(ServiceError::internal(format!(
            "signer 响应状态无效: raw={}",
            truncate(trimmed, 512)
        ))),
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
        let parsed: IndexMap<String, String> = serde_json::from_str(trimmed)
            .map_err(|error| ServiceError::internal(format!("signer 签名 JSON 解析失败: {error}")))?;
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

/// Matches Java's HEADER_COLON_PAIR = Pattern.compile("^[A-Za-z0-9-]{1,64}:\\s*.+$")
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

#[cfg(test)]
mod tests {
    use super::{encode_protocol_field, parse_sign_response};

    #[test]
    fn parse_ok_response_decodes_base64_payload() {
        let line = format!("ok\t{}\n", encode_protocol_field("a:1\r\nb:2"));
        let raw = parse_sign_response(&line).expect("ok response should decode");
        assert_eq!(raw, "a:1\r\nb:2");
    }

    #[test]
    fn parse_error_response_returns_service_error() {
        let line = format!("err\t1003\t{}\n", encode_protocol_field("signer unavailable"));
        let error = parse_sign_response(&line).expect_err("err response should fail");
        assert_eq!(error.code, 1003);
        assert_eq!(error.message, "signer unavailable");
    }
}
