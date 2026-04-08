#[path = "worker/idle_fq_native.rs"]
mod idle_fq_native;

use anyhow::{anyhow, Result};
use idle_fq_native::IdleFqNative;

const DEFAULT_RESOURCE_ROOT: &str = "/app/resources";

#[derive(Debug, Clone)]
pub struct NativeSignerConfig {
    pub verbose: bool,
    pub apk_path: Option<String>,
    pub resource_root: String,
    pub rnidbg_base_path: Option<String>,
}

impl NativeSignerConfig {
    pub fn from_env() -> Self {
        Self {
            verbose: std::env::var("UNIDBG_VERBOSE")
                .ok()
                .as_deref()
                .unwrap_or("false")
                .parse()
                .unwrap_or(false),
            apk_path: trim_to_null(std::env::var("UNIDBG_APK_PATH").ok()),
            resource_root: resolve_resource_root(),
            rnidbg_base_path: trim_to_null(std::env::var("RNIDBG_BASE_PATH").ok()),
        }
    }
}

pub struct NativeSigner {
    config: NativeSignerConfig,
    inner: IdleFqNative,
}

impl NativeSigner {
    pub fn new(config: NativeSignerConfig) -> Result<Self> {
        let inner = create_idle_fq(&config)?;
        Ok(Self { config, inner })
    }

    pub fn sign(&mut self, url: &str, headers_text: &str) -> Result<String> {
        let raw = self
            .inner
            .generate_signature(url, headers_text)?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow!("signer unavailable"))?;
        Ok(raw)
    }

    pub fn restart(&mut self) -> Result<()> {
        let replacement = create_idle_fq(&self.config)?;
        let mut previous = std::mem::replace(&mut self.inner, replacement);
        previous.destroy();
        Ok(())
    }
}

impl Drop for NativeSigner {
    fn drop(&mut self) {
        self.inner.destroy();
    }
}

fn create_idle_fq(config: &NativeSignerConfig) -> Result<IdleFqNative> {
    IdleFqNative::new(
        config.verbose,
        config.apk_path.clone(),
        config.resource_root.clone(),
        config.rnidbg_base_path.clone(),
    )
}

fn trim_to_null(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn resolve_resource_root() -> String {
    let preferred = trim_to_null(std::env::var("FQ_SIGNER_RESOURCE_ROOT").ok());
    let legacy = trim_to_null(std::env::var("UNIDBG_RESOURCE_ROOT").ok());
    preferred
        .or(legacy)
        .unwrap_or_else(|| DEFAULT_RESOURCE_ROOT.to_string())
}
