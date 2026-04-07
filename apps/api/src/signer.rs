use crate::config::SignerConfig;
use crate::models::{ServiceError, ServiceResult};
use indexmap::IndexMap;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::task;
use tracing::warn;
use uuid::Uuid;

#[derive(Clone)]
pub struct SignerClient {
    inner: Arc<Mutex<SignerProcess>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignResult {
    pub headers: IndexMap<String, String>,
}

impl SignerClient {
    pub fn new(config: SignerConfig) -> ServiceResult<Self> {
        let mut process = SignerProcess::new(config.command, config.restart_cooldown_ms);
        process.ensure_started()?;
        Ok(Self {
            inner: Arc::new(Mutex::new(process)),
        })
    }

    pub async fn sign(&self, url: &str, headers: &IndexMap<String, String>) -> ServiceResult<SignResult> {
        let headers_text = build_signature_input_headers(headers);
        let params = serde_json::to_value(SignRequest {
            url,
            headers_text: &headers_text,
        })
        .map_err(|error| ServiceError::internal(format!("signer 请求序列化失败: {error}")))?;
        let inner = self.inner.clone();
        task::spawn_blocking(move || {
            let mut process = inner
                .lock()
                .map_err(|_| ServiceError::internal("signer 进程锁异常"))?;

            match process.call::<JavaSignResult>("sign", params.clone()) {
                Ok(data) => Ok(SignResult {
                    headers: parse_signature_result(&data.raw)?,
                }),
                Err(error) => {
                    if process.should_restart_after_sign_error(&error)
                        && process.restart_if_allowed("AUTO_RESTART:SIGNER_ERROR")?
                    {
                        let data = process.call::<JavaSignResult>("sign", params)?;
                        return Ok(SignResult {
                            headers: parse_signature_result(&data.raw)?,
                        });
                    }
                    Err(error)
                }
            }
        })
        .await
        .map_err(|error| ServiceError::internal(format!("signer 请求执行失败: {error}")))?
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

    fn call<T>(&mut self, method: &str, params: Value) -> ServiceResult<T>
    where
        T: DeserializeOwned,
    {
        self.ensure_started()?;
        let request = WorkerRequest {
            id: Uuid::new_v4().to_string(),
            method: method.to_string(),
            params,
        };

        let payload = serde_json::to_string(&request)
            .map_err(|error| ServiceError::internal(format!("signer 请求编码失败: {error}")))?;

        let write_result = {
            let stdin = self
                .stdin
                .as_mut()
                .ok_or_else(|| ServiceError::internal("signer stdin 不可用"))?;
            stdin
                .write_all(payload.as_bytes())
                .and_then(|_| stdin.write_all(b"\n"))
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

        let response: WorkerResponse<T> = serde_json::from_str(&line).map_err(|error| {
            ServiceError::internal(format!(
                "signer 响应解析失败: {error}; raw={}",
                truncate(&line, 512)
            ))
        })?;

        if response.code != 0 {
            return Err(ServiceError::new(response.code, response.message));
        }

        response
            .data
            .ok_or_else(|| ServiceError::internal("signer 返回空 data"))
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

#[derive(Serialize)]
struct SignRequest<'a> {
    url: &'a str,
    headers_text: &'a str,
}

#[derive(Serialize)]
struct WorkerRequest {
    id: String,
    method: String,
    params: Value,
}

#[derive(Deserialize)]
struct WorkerResponse<T> {
    code: i32,
    message: String,
    data: Option<T>,
}

#[derive(Deserialize)]
struct JavaSignResult {
    raw: String,
}

fn truncate(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        value.to_string()
    } else {
        format!("{}...", &value[..max_len])
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as i64)
        .unwrap_or(0)
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

fn looks_like_colon_pairs(lines: &[&str]) -> bool {
    lines.iter().any(|line| {
        let trimmed = line.trim();
        let Some(index) = trimmed.find(':') else {
            return false;
        };
        index > 0 && index < trimmed.len() - 1
    })
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
