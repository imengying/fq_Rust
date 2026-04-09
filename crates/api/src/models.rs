use chrono::Utc;
use serde::{Deserialize, Deserializer, Serialize};

pub type ServiceResult<T> = Result<T, ServiceError>;

#[derive(Debug, Clone)]
pub struct ServiceError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: Option<T>,
    pub server_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    #[serde(default)]
    pub books: Vec<BookItem>,
    pub total: usize,
    pub has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BookItem {
    pub book_id: String,
    pub book_name: String,
    pub author: String,
    pub description: String,
    pub cover_url: String,
    pub last_chapter_title: String,
    pub category: String,
    #[serde(deserialize_with = "flexible_u64", default)]
    pub word_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DirectoryResponse {
    #[serde(rename = "ban_recover", skip_serializing_if = "Option::is_none")]
    pub ban_recover: Option<bool>,
    #[serde(
        rename = "additional_item_data_list",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_item_data_list: Option<serde_json::Value>,
    #[serde(rename = "catalog_data", skip_serializing_if = "Option::is_none")]
    pub catalog_data: Option<Vec<CatalogItem>>,
    #[serde(rename = "item_data_list", default)]
    pub item_data_list: Vec<DirectoryItemData>,
    #[serde(rename = "field_cache_status", skip_serializing_if = "Option::is_none")]
    pub field_cache_status: Option<serde_json::Value>,
    #[serde(rename = "book_info", skip_serializing_if = "Option::is_none")]
    pub book_info: Option<UpstreamBookInfo>,
    #[serde(
        rename = "serial_count",
        deserialize_with = "flexible_opt_i32",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub serial_count: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CatalogItem {
    #[serde(rename = "catalog_id")]
    pub catalog_id: Option<String>,
    #[serde(rename = "catalog_title")]
    pub catalog_title: Option<String>,
    #[serde(rename = "item_id")]
    pub item_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DirectoryItemData {
    #[serde(rename = "item_id")]
    pub item_id: String,
    pub title: String,
    #[serde(
        rename = "chapter_index",
        deserialize_with = "flexible_opt_i32",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub chapter_index: Option<i32>,
    #[serde(rename = "is_latest", skip_serializing_if = "Option::is_none")]
    pub is_latest: Option<bool>,
    #[serde(
        rename = "first_pass_time",
        deserialize_with = "flexible_opt_i32",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub first_pass_time: Option<i32>,
    #[serde(
        rename = "first_pass_time_str",
        skip_serializing_if = "Option::is_none"
    )]
    pub first_pass_time_str: Option<String>,
    #[serde(
        rename = "sort_order",
        deserialize_with = "flexible_opt_i32",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub sort_order: Option<i32>,
    #[serde(rename = "is_free", skip_serializing_if = "Option::is_none")]
    pub is_free: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpstreamBookInfo {
    #[serde(rename = "book_name")]
    pub book_name: Option<String>,
    pub author: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_content: Option<String>,
    #[serde(rename = "book_abstract_v2")]
    pub book_abstract_v2: Option<String>,
    #[serde(rename = "thumb_url")]
    pub thumb_url: Option<String>,
    #[serde(rename = "word_number", deserialize_with = "flexible_opt_u64", default)]
    pub word_number: Option<u64>,
    #[serde(rename = "last_chapter_title")]
    pub last_chapter_title: Option<String>,
    pub category: Option<String>,
    #[serde(deserialize_with = "flexible_opt_i32", default)]
    pub status: Option<i32>,
    #[serde(
        rename = "serial_count",
        deserialize_with = "flexible_opt_i32",
        default
    )]
    pub serial_count: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BookInfo {
    pub book_id: String,
    pub book_name: String,
    pub author: String,
    pub description: String,
    pub cover_url: String,
    #[serde(deserialize_with = "flexible_i32", default)]
    pub total_chapters: i32,
    #[serde(deserialize_with = "flexible_u64", default)]
    pub word_number: u64,
    pub last_chapter_title: String,
    pub category: String,
    #[serde(deserialize_with = "flexible_i32", default)]
    pub status: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChapterInfo {
    pub chapter_id: String,
    pub book_id: String,
    pub author_name: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_content: Option<String>,
    #[serde(
        deserialize_with = "flexible_opt_i32",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub chapter_index: Option<i32>,
    #[serde(deserialize_with = "flexible_i32", default)]
    pub word_count: i32,
    #[serde(deserialize_with = "flexible_i64", default)]
    pub update_time: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_chapter_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_chapter_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_free: Option<bool>,
    pub txt_content: String,
}

impl ServiceError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(-1, message)
    }
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: 0,
            message: "success".to_string(),
            data: Some(data),
            server_time: Utc::now().timestamp_millis(),
        }
    }

    pub fn error(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
            server_time: Utc::now().timestamp_millis(),
        }
    }
}

// ── ByteDance API 灵活反序列化辅助函数 ──
// 上游 API 经常将数字以字符串形式返回（如 "600" 而不是 600）

fn flexible_i32<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i32, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    match &value {
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(|v| v as i32)
            .ok_or_else(|| serde::de::Error::custom("invalid number")),
        serde_json::Value::String(s) => s.parse::<i32>().map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom(format!(
            "expected number or string, got {value}"
        ))),
    }
}

fn flexible_i64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    match &value {
        serde_json::Value::Number(n) => n
            .as_i64()
            .ok_or_else(|| serde::de::Error::custom("invalid number")),
        serde_json::Value::String(s) => s.parse::<i64>().map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom(format!(
            "expected number or string, got {value}"
        ))),
    }
}

fn flexible_u64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    match &value {
        serde_json::Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| serde::de::Error::custom("invalid number")),
        serde_json::Value::String(s) => s.parse::<u64>().map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom(format!(
            "expected number or string, got {value}"
        ))),
    }
}

fn flexible_opt_i32<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i32>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => Ok(Some(n.as_i64().unwrap_or(0) as i32)),
        Some(serde_json::Value::String(s)) if s.is_empty() => Ok(None),
        Some(serde_json::Value::String(s)) => {
            s.parse::<i32>().map(Some).map_err(serde::de::Error::custom)
        }
        Some(other) => Err(serde::de::Error::custom(format!(
            "expected number or string, got {other}"
        ))),
    }
}

fn flexible_opt_u64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => Ok(Some(n.as_u64().unwrap_or(0))),
        Some(serde_json::Value::String(s)) if s.is_empty() => Ok(None),
        Some(serde_json::Value::String(s)) => {
            s.parse::<u64>().map(Some).map_err(serde::de::Error::custom)
        }
        Some(other) => Err(serde::de::Error::custom(format!(
            "expected number or string, got {other}"
        ))),
    }
}
