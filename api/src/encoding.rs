use anyhow::{anyhow, Result};
use flate2::read::GzDecoder;
use std::io::Read;

pub fn decode_hex_16(input: &str) -> Result<Vec<u8>> {
    let normalized = input.trim();
    if normalized.len() != 32 {
        return Err(anyhow!("Key length mismatch"));
    }
    let mut bytes = Vec::with_capacity(16);
    let chars: Vec<char> = normalized.chars().collect();
    for pair in chars.chunks(2) {
        let hi = pair
            .first()
            .and_then(|value| value.to_digit(16))
            .ok_or_else(|| anyhow!("非法十六进制 key"))?;
        let lo = pair
            .get(1)
            .and_then(|value| value.to_digit(16))
            .ok_or_else(|| anyhow!("非法十六进制 key"))?;
        bytes.push(((hi << 4) | lo) as u8);
    }
    Ok(bytes)
}

pub fn decode_gzip_or_utf8(raw: &[u8]) -> Result<String> {
    if looks_like_gzip(raw) {
        let mut decoder = GzDecoder::new(raw);
        let mut output = String::new();
        decoder.read_to_string(&mut output)?;
        return Ok(output);
    }

    Ok(String::from_utf8_lossy(raw).to_string())
}

pub fn decode_upstream_response(raw: &[u8], content_encoding: Option<&str>) -> Result<String> {
    let header_declares_gzip = content_encoding
        .map(|value| value.to_ascii_lowercase().contains("gzip"))
        .unwrap_or(false);

    if header_declares_gzip || looks_like_gzip(raw) {
        match decode_gzip_or_utf8(raw) {
            Ok(value) => return Ok(value),
            Err(_) => return Ok(String::from_utf8_lossy(raw).to_string()),
        }
    }

    Ok(String::from_utf8_lossy(raw).to_string())
}

fn looks_like_gzip(raw: &[u8]) -> bool {
    raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b
}
