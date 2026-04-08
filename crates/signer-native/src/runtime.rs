#[path = "worker/idle_fq_native.rs"]
mod idle_fq_native;

use anyhow::{anyhow, Result};
use idle_fq_native::IdleFqNative;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Debug)]
struct EmbeddedRuntimeLayout {
    resource_root: PathBuf,
    sdk_root: PathBuf,
}

struct EmbeddedFile {
    relative_path: &'static str,
    bytes: &'static [u8],
    executable: bool,
}

static EMBEDDED_RUNTIME: OnceLock<std::result::Result<EmbeddedRuntimeLayout, String>> =
    OnceLock::new();

const EMBEDDED_RESOURCE_FILES: &[EmbeddedFile] = &[
    EmbeddedFile {
        relative_path: "com/dragon/read/oversea/gp/apk/base.apk",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/fq-signer/com/dragon/read/oversea/gp/apk/base.apk"
        )),
        executable: false,
    },
    EmbeddedFile {
        relative_path: "com/dragon/read/oversea/gp/lib/libc++_shared.so",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/fq-signer/com/dragon/read/oversea/gp/lib/libc++_shared.so"
        )),
        executable: false,
    },
    EmbeddedFile {
        relative_path: "com/dragon/read/oversea/gp/lib/libmetasec_ml.so",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/fq-signer/com/dragon/read/oversea/gp/lib/libmetasec_ml.so"
        )),
        executable: false,
    },
    EmbeddedFile {
        relative_path: "com/dragon/read/oversea/gp/other/ms_16777218.bin",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/fq-signer/com/dragon/read/oversea/gp/other/ms_16777218.bin"
        )),
        executable: false,
    },
];

const EMBEDDED_SDK_FILES: &[EmbeddedFile] = &[
    EmbeddedFile {
        relative_path: "system/bin/ls",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/rnidbg/android/sdk23/system/bin/ls"
        )),
        executable: true,
    },
    EmbeddedFile {
        relative_path: "system/bin/sh",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/rnidbg/android/sdk23/system/bin/sh"
        )),
        executable: true,
    },
    EmbeddedFile {
        relative_path: "system/lib64/libc++.so",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/rnidbg/android/sdk23/system/lib64/libc++.so"
        )),
        executable: false,
    },
    EmbeddedFile {
        relative_path: "system/lib64/libc.so",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/rnidbg/android/sdk23/system/lib64/libc.so"
        )),
        executable: false,
    },
    EmbeddedFile {
        relative_path: "system/lib64/libcrypto.so",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/rnidbg/android/sdk23/system/lib64/libcrypto.so"
        )),
        executable: false,
    },
    EmbeddedFile {
        relative_path: "system/lib64/libdl.so",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/rnidbg/android/sdk23/system/lib64/libdl.so"
        )),
        executable: false,
    },
    EmbeddedFile {
        relative_path: "system/lib64/liblog.so",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/rnidbg/android/sdk23/system/lib64/liblog.so"
        )),
        executable: false,
    },
    EmbeddedFile {
        relative_path: "system/lib64/libm.so",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/rnidbg/android/sdk23/system/lib64/libm.so"
        )),
        executable: false,
    },
    EmbeddedFile {
        relative_path: "system/lib64/libssl.so",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/rnidbg/android/sdk23/system/lib64/libssl.so"
        )),
        executable: false,
    },
    EmbeddedFile {
        relative_path: "system/lib64/libstdc++.so",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/rnidbg/android/sdk23/system/lib64/libstdc++.so"
        )),
        executable: false,
    },
    EmbeddedFile {
        relative_path: "system/lib64/libz.so",
        bytes: include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/rnidbg/android/sdk23/system/lib64/libz.so"
        )),
        executable: false,
    },
];

#[derive(Debug, Clone)]
pub struct NativeSignerConfig {
    pub verbose: bool,
    pub apk_path: Option<String>,
    pub resource_root: String,
    pub rnidbg_base_path: Option<String>,
    pub android_sdk_api: u32,
}

impl NativeSignerConfig {
    pub fn from_env(android_sdk_api: u32) -> Result<Self> {
        let embedded = materialize_embedded_runtime()?;
        let preferred_sdk_root = resolve_rnidbg_base_path(&embedded.sdk_root);
        Ok(Self {
            verbose: std::env::var("UNIDBG_VERBOSE")
                .ok()
                .as_deref()
                .unwrap_or("false")
                .parse()
                .unwrap_or(false),
            apk_path: trim_to_null(std::env::var("UNIDBG_APK_PATH").ok()),
            resource_root: resolve_resource_root(),
            rnidbg_base_path: Some(preferred_sdk_root.to_string_lossy().to_string()),
            android_sdk_api,
        })
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
        config.android_sdk_api,
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
    if let Some(path) = preferred.or(legacy) {
        return path;
    }

    materialize_embedded_runtime()
        .map(|layout| layout.resource_root.to_string_lossy().to_string())
        .unwrap_or_else(|_| std::env::temp_dir().join("fq-rust-embedded-runtime/resources").to_string_lossy().to_string())
}

fn resolve_rnidbg_base_path(fallback: &Path) -> PathBuf {
    if let Some(path) = trim_to_null(std::env::var("RNIDBG_BASE_PATH").ok()) {
        return PathBuf::from(path);
    }

    let local_sdk31 = PathBuf::from("local/rnidbg/sdk31");
    if local_sdk31.join("system/lib64/libc.so").exists() {
        return local_sdk31;
    }

    fallback.to_path_buf()
}

fn materialize_embedded_runtime() -> Result<EmbeddedRuntimeLayout> {
    match EMBEDDED_RUNTIME.get_or_init(|| {
        materialize_embedded_runtime_once().map_err(|error| error.to_string())
    }) {
        Ok(layout) => Ok(layout.clone()),
        Err(error) => Err(anyhow!(error.clone())),
    }
}

fn materialize_embedded_runtime_once() -> Result<EmbeddedRuntimeLayout> {
    let root = std::env::temp_dir().join("fq-rust-embedded-runtime");
    let resource_root = root.join("resources");
    let sdk_root = root.join("sdk-runtime");

    write_embedded_files(&resource_root, EMBEDDED_RESOURCE_FILES)?;
    write_embedded_files(&sdk_root, EMBEDDED_SDK_FILES)?;

    Ok(EmbeddedRuntimeLayout {
        resource_root,
        sdk_root,
    })
}

fn write_embedded_files(base: &Path, files: &[EmbeddedFile]) -> Result<()> {
    for file in files {
        let target = base.join(file.relative_path);
        if needs_refresh(&target, file.bytes)? {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target, file.bytes)?;
        }
        apply_permissions(&target, file.executable)?;
    }
    Ok(())
}

fn needs_refresh(path: &Path, bytes: &[u8]) -> Result<bool> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(metadata.len() != bytes.len() as u64),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error.into()),
    }
}

fn apply_permissions(path: &Path, executable: bool) -> Result<()> {
    #[cfg(unix)]
    {
        let mode = if executable { 0o755 } else { 0o644 };
        let permissions = std::fs::Permissions::from_mode(mode);
        std::fs::set_permissions(path, permissions)?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, executable);
    }
    Ok(())
}
