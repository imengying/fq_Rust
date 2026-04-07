use crate::config::DeviceProfile;
use crate::models::{ServiceError, ServiceResult};
use indexmap::IndexMap;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::task;
use uuid::Uuid;

#[derive(Clone)]
pub struct SidecarClient {
    inner: Arc<Mutex<SidecarProcess>>,
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
    pub fn new(command: Vec<String>) -> ServiceResult<Self> {
        let mut process = SidecarProcess::new(command);
        process.ensure_started()?;
        Ok(Self {
            inner: Arc::new(Mutex::new(process)),
        })
    }

    pub async fn sign(&self, url: &str, headers: &IndexMap<String, String>) -> ServiceResult<SignResult> {
        let params = serde_json::to_value(SignRequest { url, headers })
            .map_err(|error| ServiceError::internal(format!("sidecar 请求序列化失败: {error}")))?;
        self.request("sign", params).await
    }

    pub async fn resolve_register_key(
        &self,
        device_profile: &DeviceProfile,
        required_keyver: Option<i64>,
    ) -> ServiceResult<RegisterKeyResolveResult> {
        let params = serde_json::to_value(RegisterKeyResolveRequest {
            device_profile,
            required_keyver,
        })
        .map_err(|error| ServiceError::internal(format!("sidecar 请求序列化失败: {error}")))?;
        self.request("register-key-resolve", params).await
    }

    pub async fn invalidate_register_key(&self, device_fingerprint: &str) -> ServiceResult<()> {
        let params = serde_json::to_value(RegisterKeyInvalidateRequest { device_fingerprint })
            .map_err(|error| ServiceError::internal(format!("sidecar 请求序列化失败: {error}")))?;
        let _: RegisterKeyInvalidateResult = self.request("register-key-invalidate", params).await?;
        Ok(())
    }

    async fn request<T>(&self, method: &str, params: Value) -> ServiceResult<T>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let method = method.to_string();
        let inner = self.inner.clone();
        task::spawn_blocking(move || {
            let mut process = inner
                .lock()
                .map_err(|_| ServiceError::internal("sidecar 进程锁异常"))?;
            process.call(&method, params)
        })
        .await
        .map_err(|error| ServiceError::internal(format!("sidecar 请求执行失败: {error}")))?
    }
}

struct SidecarProcess {
    command: Vec<String>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
}

impl SidecarProcess {
    fn new(command: Vec<String>) -> Self {
        Self {
            command,
            child: None,
            stdin: None,
            stdout: None,
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
            .map_err(|error| ServiceError::internal(format!("sidecar 请求编码失败: {error}")))?;

        let write_result = {
            let stdin = self
                .stdin
                .as_mut()
                .ok_or_else(|| ServiceError::internal("sidecar stdin 不可用"))?;
            stdin
                .write_all(payload.as_bytes())
                .and_then(|_| stdin.write_all(b"\n"))
                .and_then(|_| stdin.flush())
        };
        if let Err(error) = write_result {
            self.reset_process();
            return Err(ServiceError::internal(format!("sidecar 请求写入失败: {error}")));
        }

        let mut line = String::new();
        let read_result = {
            let stdout = self
                .stdout
                .as_mut()
                .ok_or_else(|| ServiceError::internal("sidecar stdout 不可用"))?;
            stdout.read_line(&mut line)
        };
        let read = match read_result {
            Ok(value) => value,
            Err(error) => {
                self.reset_process();
                return Err(ServiceError::internal(format!("sidecar 响应读取失败: {error}")));
            }
        };
        if read == 0 {
            self.reset_process();
            return Err(ServiceError::internal("sidecar 已退出"));
        }

        let response: WorkerResponse<T> = serde_json::from_str(&line).map_err(|error| {
            ServiceError::internal(format!(
                "sidecar 响应解析失败: {error}; raw={}",
                truncate(&line, 512)
            ))
        })?;

        if response.code != 0 {
            return Err(ServiceError::new(response.code, response.message));
        }

        response
            .data
            .ok_or_else(|| ServiceError::internal("sidecar 返回空 data"))
    }

    fn ensure_started(&mut self) -> ServiceResult<()> {
        if let Some(child) = self.child.as_mut() {
            if child
                .try_wait()
                .map_err(|error| ServiceError::internal(format!("sidecar 状态检查失败: {error}")))?
                .is_none()
            {
                return Ok(());
            }
            self.reset_process();
        }

        let binary = self
            .command
            .first()
            .ok_or_else(|| ServiceError::internal("sidecar.command 不能为空"))?
            .clone();
        let mut command = Command::new(binary);
        if self.command.len() > 1 {
            command.args(&self.command[1..]);
        }
        command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());

        let mut child = command
            .spawn()
            .map_err(|error| ServiceError::internal(format!("sidecar 启动失败: {error}")))?;
        self.stdin = child.stdin.take();
        self.stdout = child.stdout.take().map(BufReader::new);
        self.child = Some(child);
        Ok(())
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

impl Drop for SidecarProcess {
    fn drop(&mut self) {
        self.reset_process();
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
struct RegisterKeyInvalidateResult {
    invalidated: bool,
}

fn truncate(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        value.to_string()
    } else {
        format!("{}...", &value[..max_len])
    }
}
