mod idle_fq_native;

use anyhow::Result;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use idle_fq_native::IdleFqNative;
use std::io::{BufRead, BufReader, BufWriter, Write};

const DEFAULT_RESOURCE_ROOT: &str = "/app/unidbg";

pub fn run() -> Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .init();

    let mut signer = IdleFqNative::new(
        std::env::var("UNIDBG_VERBOSE")
            .ok()
            .as_deref()
            .unwrap_or("false")
            .parse()
            .unwrap_or(false),
        trim_to_null(std::env::var("UNIDBG_APK_PATH").ok()),
        default_if_null(
            trim_to_null(std::env::var("UNIDBG_RESOURCE_ROOT").ok()),
            DEFAULT_RESOURCE_ROOT,
        ),
        trim_to_null(std::env::var("RNIDBG_BASE_PATH").ok()),
    )?;

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let response = handle(line.trim_end_matches(['\r', '\n']), &mut signer);
        writer.write_all(response.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()?;
    }

    signer.destroy();
    Ok(())
}

fn handle(line: &str, signer: &mut IdleFqNative) -> String {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() != 3 || parts[0] != "sign" {
        return encode_error(1001, "invalid request");
    }

    let url = match decode_field(parts[1], "url") {
        Ok(value) => value,
        Err(message) => return encode_error(1001, &message),
    };
    let headers_text = match decode_field(parts[2], "headers_text") {
        Ok(value) => value,
        Err(message) => return encode_error(1001, &message),
    };

    if url.trim().is_empty() {
        return encode_error(1001, "url 不能为空");
    }

    match signer.generate_signature(&url, &headers_text) {
        Ok(Some(raw)) if !raw.trim().is_empty() => format!("ok\t{}", encode_field(&raw)),
        Ok(_) => encode_error(1003, "signer unavailable"),
        Err(error) => {
            eprintln!("rust signer request failed: {error:?}");
            encode_error(1500, "internal signer error")
        }
    }
}

fn encode_error(code: i32, message: &str) -> String {
    format!("err\t{code}\t{}", encode_field(message))
}

fn encode_field(value: &str) -> String {
    URL_SAFE_NO_PAD.encode(value.as_bytes())
}

fn decode_field(value: &str, field_name: &str) -> std::result::Result<String, String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| format!("{field_name} 非法"))?;
    String::from_utf8(bytes).map_err(|_| format!("{field_name} 非法"))
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

fn default_if_null(value: Option<String>, default: &str) -> String {
    value.unwrap_or_else(|| default.to_string())
}
