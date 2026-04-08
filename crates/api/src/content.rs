use aes::Aes128;
use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockDecryptMut, KeyIvInit};
use crate::encoding::{decode_gzip_or_utf8, decode_hex_16};
use once_cell::sync::Lazy;
use regex::Regex;

type Aes128CbcDec = cbc::Decryptor<Aes128>;

static BLK_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<blk[^>]*>([^<]*)</blk>").expect("valid blk regex"));
static TITLE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?is)<h1[^>]*>.*?<blk[^>]*>([^<]*)</blk>.*?</h1>").expect("valid title regex")
});
static TAG_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<[^>]+>").expect("valid tag regex"));

pub fn decrypt_and_decompress_content(encrypted_content: &str, key_hex: &str) -> Result<String> {
    let raw = BASE64
        .decode(encrypted_content)
        .map_err(|error| anyhow!("章节内容 Base64 解码失败: {error}"))?;
    if raw.len() < 16 {
        return Err(anyhow!("Encrypted data too short"));
    }

    let (iv, ciphertext) = raw.split_at(16);
    let key = decode_hex_16(key_hex)?;
    let mut buffer = ciphertext.to_vec();
    let decrypted = Aes128CbcDec::new_from_slices(&key, iv)
        .map_err(|error| anyhow!("AES 初始化失败: {error}"))?
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|error| anyhow!("章节解密失败: {error}"))?;

    decode_gzip_or_utf8(decrypted)
}

pub fn extract_text(html_content: &str) -> String {
    if html_content.trim().is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    for captures in BLK_RE.captures_iter(html_content) {
        if let Some(value) = captures.get(1) {
            let trimmed = value.as_str().trim();
            if !trimmed.is_empty() {
                lines.push(trimmed.to_string());
            }
        }
    }

    if lines.is_empty() {
        return TAG_RE.replace_all(html_content, "").trim().to_string();
    }

    lines.join("\n")
}

pub fn extract_title(html_content: &str) -> Option<String> {
    let capture = TITLE_RE.captures(html_content)?;
    let title = capture.get(1)?.as_str().trim().to_string();
    if title.is_empty() {
        None
    } else {
        Some(title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::Aes128;
    use cbc::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};

    type Aes128CbcEnc = cbc::Encryptor<Aes128>;

    #[test]
    fn extracts_blk_text_and_title() {
        let html = "<h1><blk>第一章</blk></h1><p><blk>内容一</blk></p><p><blk>内容二</blk></p>";
        assert_eq!(extract_title(html).as_deref(), Some("第一章"));
        assert_eq!(extract_text(html), "第一章\n内容一\n内容二");
    }

    #[test]
    fn decrypts_base64_aes_payload() {
        let key_hex = "0123456789ABCDEF0123456789ABCDEF";
        let key = decode_hex_16(key_hex).unwrap();
        let iv = [7u8; 16];
        let plaintext = b"plain text payload";
        let mut buffer = [0u8; 64];
        buffer[..plaintext.len()].copy_from_slice(plaintext);
        let ciphertext = Aes128CbcEnc::new_from_slices(&key, &iv)
            .unwrap()
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
            .unwrap()
            .to_vec();

        let mut payload = iv.to_vec();
        payload.extend(ciphertext);
        let encoded = BASE64.encode(payload);

        let decoded = decrypt_and_decompress_content(&encoded, key_hex).unwrap();
        assert_eq!(decoded, "plain text payload");
    }
}
