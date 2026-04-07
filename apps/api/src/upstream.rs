use crate::config::{DeviceProfile, UpstreamDevice};
use crate::content::{decrypt_and_decompress_content, extract_text, extract_title};
use crate::models::{
    BookInfo, BookItem, ChapterInfo, DirectoryItemData, DirectoryResponse, SearchResponse,
    ServiceError, ServiceResult, UpstreamBookInfo,
};
use crate::registerkey::RegisterKeyResolveResult;
use crate::state::AppState;
use indexmap::IndexMap;
use rand::Rng;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use url::Url;
use uuid::Uuid;

const SEARCH_PATH: &str = "/reading/bookapi/search/tab/v";
const DIRECTORY_PATH: &str = "/reading/bookapi/directory/all_items/v";
const BATCH_FULL_PATH: &str = "/reading/reader/batch_full/v";

pub async fn search_books(
    state: &AppState,
    key: String,
    page: usize,
    size: usize,
    tab_type: u32,
    search_id: Option<String>,
) -> ServiceResult<SearchResponse> {
    let cache_key = format!(
        "{}|{}|{}|{}|{}",
        key.trim(),
        page,
        size,
        tab_type,
        search_id.clone().unwrap_or_default()
    );
    if let Some(cached) = state.search_cache.get(&cache_key) {
        return Ok(cached);
    }

    let offset = page.saturating_sub(1).saturating_mul(size);
    let mut request = SearchUpstreamRequest::new(
        key.clone(),
        offset,
        size,
        tab_type,
        search_id.clone(),
        &state.config.fq.device_profile.device,
    );

    let response = if request.search_id.is_some() {
        execute_search_once(state, &request).await?
    } else {
        request.is_first_enter_search = true;
        request.last_search_page_interval = 0;
        let first_response = execute_search_once(state, &request).await?;
        if first_response.search_id.is_some() || !first_response.books.is_empty() {
            if let Some(search_id) = first_response.search_id.clone() {
                let delay_ms = bounded_delay(
                    state.config.fq.search.phase1_delay_min_ms,
                    state.config.fq.search.phase1_delay_max_ms,
                );
                sleep(Duration::from_millis(delay_ms)).await;
                let mut second_request = request.clone();
                second_request.search_id = Some(search_id.clone());
                second_request.is_first_enter_search = false;
                second_request.last_search_page_interval = delay_ms as i32;
                let mut second_response = execute_search_once(state, &second_request).await?;
                if second_response.search_id.is_none() {
                    second_response.search_id = Some(search_id);
                }
                second_response
            } else {
                first_response
            }
        } else {
            first_response
        }
    };

    state.search_cache.insert(cache_key, response.clone());
    Ok(response)
}

pub async fn get_toc(state: &AppState, book_id: &str) -> ServiceResult<DirectoryResponse> {
    fetch_directory(state, book_id, true).await
}

pub async fn get_book_info(state: &AppState, book_id: &str) -> ServiceResult<BookInfo> {
    let cache_key = format!("book:{book_id}");
    if let Some(cached) = state.book_cache.get(&cache_key) {
        return Ok(cached);
    }

    let directory = fetch_directory(state, book_id, false).await?;
    let book_info = build_book_info(book_id, &directory)?;
    state.book_cache.insert(cache_key, book_info.clone());
    Ok(book_info)
}

pub async fn get_chapter(state: &AppState, book_id: &str, chapter_id: &str) -> ServiceResult<ChapterInfo> {
    let cache_key = format!("chapter:{book_id}:{chapter_id}");
    if let Some(cached) = state.chapter_cache.get(&cache_key) {
        return Ok(cached);
    }

    let directory = fetch_directory(state, book_id, true).await.ok();
    let batch_response = fetch_batch_full(state, book_id, &[chapter_id.to_string()]).await?;
    let item_content = batch_response
        .data
        .get(chapter_id)
        .cloned()
        .ok_or_else(|| ServiceError::internal("上游未返回目标章节"))?;

    let content = item_content
        .content
        .clone()
        .ok_or_else(|| ServiceError::internal("章节内容为空/过短"))?;
    let resolve = resolve_register_key(state, item_content.key_version).await?;
    let html = decrypt_chapter_with_retry(state, &content, item_content.key_version, &resolve).await?;
    let text = extract_text(&html);
    if text.trim().is_empty() {
        return Err(ServiceError::internal("章节内容为空/过短"));
    }

    let context = directory
        .as_ref()
        .and_then(|value| chapter_context(value, chapter_id));
    let title = first_non_blank(&[
        item_content.title.as_deref().unwrap_or_default(),
        context
            .as_ref()
            .and_then(|value| value.title.as_deref())
            .unwrap_or_default(),
        extract_title(&html).as_deref().unwrap_or(""),
        "章节标题",
    ]);
    let author = item_content
        .novel_data
        .as_ref()
        .and_then(|value| value.author.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("未知作者")
        .to_string();

    let chapter = ChapterInfo {
        chapter_id: chapter_id.to_string(),
        book_id: book_id.to_string(),
        author_name: author,
        title,
        raw_content: None,
        chapter_index: context.as_ref().map(|value| value.chapter_index),
        word_count: text.chars().count() as i32,
        update_time: now_ms(),
        prev_chapter_id: context.as_ref().and_then(|value| value.prev_chapter_id.clone()),
        next_chapter_id: context.as_ref().and_then(|value| value.next_chapter_id.clone()),
        is_free: context.as_ref().map(|value| value.is_free),
        txt_content: text,
    };

    state.chapter_cache.insert(cache_key, chapter.clone());
    Ok(chapter)
}

async fn fetch_directory(state: &AppState, book_id: &str, minimal: bool) -> ServiceResult<DirectoryResponse> {
    let cache_key = format!("directory:{}:{book_id}", if minimal { "min" } else { "full" });
    if let Some(cached) = state.directory_cache.get(&cache_key) {
        return Ok(cached);
    }

    let device = &state.config.fq.device_profile;
    let params = build_directory_params(device, book_id, minimal);
    let url = build_url(
        &state.config.fq.upstream.resolved_search_base_url(),
        DIRECTORY_PATH,
        &params,
    )?;
    let root = execute_signed_json_get(state, &url, build_common_headers(device)).await?;
    let upstream_code = root.get("code").and_then(Value::as_i64).unwrap_or_default();
    if upstream_code != 0 {
        let message = root
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("目录接口失败");
        return Err(ServiceError::new(upstream_code as i32, message));
    }

    let data = root
        .get("data")
        .ok_or_else(|| ServiceError::internal("目录接口缺少 data"))?;
    let mut directory: DirectoryResponse = serde_json::from_value(data.clone())
        .map_err(|error| ServiceError::internal(format!("目录响应解析失败: {error}")))?;
    if minimal {
        trim_directory_for_minimal(&mut directory);
    }

    state.directory_cache.insert(cache_key, directory.clone());
    Ok(directory)
}

async fn execute_search_once(state: &AppState, request: &SearchUpstreamRequest) -> ServiceResult<SearchResponse> {
    let device = &state.config.fq.device_profile;
    let params = build_search_params(device, request);
    let url = build_url(
        &state.config.fq.upstream.resolved_search_base_url(),
        SEARCH_PATH,
        &params,
    )?;
    let root = execute_signed_json_get(state, &url, build_search_headers(device)).await?;
    let upstream_code = root.get("code").and_then(Value::as_i64).unwrap_or_default();
    if upstream_code != 0 {
        let message = root
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("搜索接口失败");
        return Err(ServiceError::new(upstream_code as i32, message));
    }

    Ok(parse_search_response(&root, request.tab_type))
}

async fn fetch_batch_full(
    state: &AppState,
    book_id: &str,
    item_ids: &[String],
) -> ServiceResult<BatchFullResponse> {
    let device = &state.config.fq.device_profile;
    let params = build_batch_full_params(device, item_ids, book_id);
    let url = build_url(&state.config.fq.upstream.base_url, BATCH_FULL_PATH, &params)?;
    let root = execute_signed_json_get(state, &url, build_common_headers(device)).await?;
    let parsed: BatchFullResponse = serde_json::from_value(root.clone())
        .map_err(|error| ServiceError::internal(format!("章节响应解析失败: {error}")))?;
    if parsed.code != 0 {
        return Err(ServiceError::new(parsed.code as i32, parsed.message));
    }
    Ok(parsed)
}

async fn resolve_register_key(
    state: &AppState,
    required_keyver: Option<i64>,
) -> ServiceResult<RegisterKeyResolveResult> {
    state
        .register_key_service
        .resolve(
            &state.http_client,
            &state.sidecar_client,
            &state.config.fq.upstream,
            &state.config.fq.device_profile,
            required_keyver,
        )
        .await
}

async fn decrypt_chapter_with_retry(
    state: &AppState,
    encrypted: &str,
    required_keyver: Option<i64>,
    first_resolve: &RegisterKeyResolveResult,
) -> ServiceResult<String> {
    match decrypt_and_decompress_content(encrypted, &first_resolve.real_key_hex) {
        Ok(value) => Ok(value),
        Err(_) => {
            state
                .register_key_service
                .invalidate(&first_resolve.device_fingerprint)?;
            let refreshed = resolve_register_key(state, required_keyver).await?;
            decrypt_and_decompress_content(encrypted, &refreshed.real_key_hex)
                .map_err(|error| ServiceError::internal(format!("章节解密失败: {error}")))
        }
    }
}

async fn execute_signed_json_get(
    state: &AppState,
    url: &str,
    headers: IndexMap<String, String>,
) -> ServiceResult<Value> {
    let sign = state.sidecar_client.sign(url, &headers).await?;
    let response = state
        .http_client
        .get(url)
        .headers(merge_headers(&headers, &sign.headers)?)
        .send()
        .await
        .map_err(|error| ServiceError::internal(format!("上游请求失败: {error}")))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| ServiceError::internal(format!("上游响应读取失败: {error}")))?;
    if body.trim().is_empty() {
        return Err(ServiceError::internal("上游返回空响应"));
    }
    if !status.is_success() {
        return Err(ServiceError::internal(format!(
            "上游 HTTP 状态异常: {}",
            status.as_u16()
        )));
    }

    serde_json::from_str(&body)
        .map_err(|error| ServiceError::internal(format!("上游 JSON 解析失败: {error}")))
}

fn merge_headers(
    original: &IndexMap<String, String>,
    signed: &IndexMap<String, String>,
) -> ServiceResult<HeaderMap> {
    let mut headers = HeaderMap::new();
    for (key, value) in original.iter().chain(signed.iter()) {
        let header_name = HeaderName::from_bytes(key.as_bytes()).map_err(|error| {
            ServiceError::internal(format!("非法请求头名称 {key}: {error}"))
        })?;
        let header_value = HeaderValue::from_str(value).map_err(|error| {
            ServiceError::internal(format!("非法请求头值 {key}: {error}"))
        })?;
        headers.insert(header_name, header_value);
    }
    Ok(headers)
}

fn build_common_headers(device: &DeviceProfile) -> IndexMap<String, String> {
    let now = now_ms();
    let mut headers = IndexMap::new();
    headers.insert(
        "accept".to_string(),
        "application/json; charset=utf-8,application/x-protobuf".to_string(),
    );
    headers.insert(
        "cookie".to_string(),
        normalize_install_id(&device.cookie, &device.device.install_id),
    );
    headers.insert("user-agent".to_string(), device.user_agent.clone());
    headers.insert("accept-encoding".to_string(), "gzip".to_string());
    headers.insert("x-xs-from-web".to_string(), "0".to_string());
    headers.insert(
        "x-vc-bdturing-sdk-version".to_string(),
        "3.7.2.cn".to_string(),
    );
    headers.insert(
        "x-reading-request".to_string(),
        format!("{}-{}", now, rand::thread_rng().gen_range(1..2_000_000_000u32)),
    );
    headers.insert("sdk-version".to_string(), "2".to_string());
    headers.insert("x-tt-store-region-src".to_string(), "did".to_string());
    headers.insert("x-tt-store-region".to_string(), "cn-zj".to_string());
    headers.insert("lc".to_string(), "101".to_string());
    headers.insert("x-ss-req-ticket".to_string(), now.to_string());
    headers.insert("passport-sdk-version".to_string(), "50564".to_string());
    headers.insert("x-ss-dp".to_string(), device.device.aid.clone());
    headers
}

fn build_search_headers(device: &DeviceProfile) -> IndexMap<String, String> {
    let headers = build_common_headers(device);
    let mut ordered = IndexMap::new();
    for (key, value) in headers {
        let is_reading_request = key.eq_ignore_ascii_case("x-reading-request");
        ordered.insert(key, value);
        if is_reading_request {
            ordered.insert("authorization".to_string(), "Bearer".to_string());
        }
    }
    if !ordered.contains_key("authorization") {
        ordered.insert("authorization".to_string(), "Bearer".to_string());
    }
    ordered
}

fn build_search_params(device: &DeviceProfile, request: &SearchUpstreamRequest) -> Vec<(String, String)> {
    let mut params = build_common_params(device);
    params.extend([
        (
            "bookshelf_search_plan".to_string(),
            request.bookshelf_search_plan.to_string(),
        ),
        ("offset".to_string(), request.offset.to_string()),
        ("from_rs".to_string(), bool01(request.from_rs)),
        ("user_is_login".to_string(), request.user_is_login.to_string()),
        ("bookstore_tab".to_string(), request.bookstore_tab.to_string()),
        ("query".to_string(), request.query.clone()),
        ("count".to_string(), request.count.to_string()),
        ("search_source".to_string(), request.search_source.to_string()),
        ("clicked_content".to_string(), request.clicked_content.clone()),
        ("search_source_id".to_string(), request.search_source_id.clone()),
        ("use_lynx".to_string(), bool01(request.use_lynx)),
        ("use_correct".to_string(), bool01(request.use_correct)),
        (
            "last_search_page_interval".to_string(),
            request.last_search_page_interval.to_string(),
        ),
        ("line_words_num".to_string(), request.line_words_num.to_string()),
        ("tab_name".to_string(), request.tab_name.clone()),
        (
            "last_consume_interval".to_string(),
            request.last_consume_interval.to_string(),
        ),
        (
            "pad_column_cover".to_string(),
            request.pad_column_cover.to_string(),
        ),
        (
            "is_first_enter_search".to_string(),
            bool01(request.is_first_enter_search),
        ),
    ]);
    if let Some(search_id) = &request.search_id {
        if !search_id.trim().is_empty() {
            params.push(("search_id".to_string(), search_id.clone()));
        }
    }
    if request.is_first_enter_search {
        params.push(("client_ab_info".to_string(), request.client_ab_info.clone()));
    }
    params.extend([
        (
            "normal_session_id".to_string(),
            request.normal_session_id.clone(),
        ),
        (
            "cold_start_session_id".to_string(),
            request.cold_start_session_id.clone(),
        ),
        ("charging".to_string(), request.charging.to_string()),
        (
            "screen_brightness".to_string(),
            request.screen_brightness.to_string(),
        ),
        ("battery_pct".to_string(), request.battery_pct.to_string()),
        ("down_speed".to_string(), request.down_speed.to_string()),
        ("sys_dark_mode".to_string(), request.sys_dark_mode.to_string()),
        ("app_dark_mode".to_string(), request.app_dark_mode.to_string()),
        ("font_scale".to_string(), request.font_scale.to_string()),
        (
            "is_android_pad_screen".to_string(),
            request.is_android_pad_screen.to_string(),
        ),
        ("network_type".to_string(), request.network_type.to_string()),
        ("current_volume".to_string(), request.current_volume.to_string()),
        ("tab_type".to_string(), request.tab_type.to_string()),
        ("passback".to_string(), request.passback.to_string()),
    ]);
    params
}

fn build_directory_params(
    device: &DeviceProfile,
    book_id: &str,
    minimal: bool,
) -> Vec<(String, String)> {
    let mut params = build_common_params(device);
    params.push(("book_type".to_string(), "0".to_string()));
    params.push(("book_id".to_string(), book_id.to_string()));
    params.push((
        "need_version".to_string(),
        (!minimal).to_string(),
    ));
    params
}

fn build_batch_full_params(
    device: &DeviceProfile,
    item_ids: &[String],
    book_id: &str,
) -> Vec<(String, String)> {
    let mut params = build_common_params(device);
    params.push(("item_ids".to_string(), item_ids.join(",")));
    params.push(("key_register_ts".to_string(), "0".to_string()));
    params.push(("book_id".to_string(), book_id.to_string()));
    params.push(("req_type".to_string(), "0".to_string()));
    params
}

fn build_common_params(device: &DeviceProfile) -> Vec<(String, String)> {
    let now = now_ms();
    vec![
        ("iid".to_string(), device.device.install_id.clone()),
        ("device_id".to_string(), device.device.device_id.clone()),
        ("ac".to_string(), "wifi".to_string()),
        ("channel".to_string(), "googleplay".to_string()),
        ("aid".to_string(), device.device.aid.clone()),
        ("app_name".to_string(), "novelapp".to_string()),
        ("version_code".to_string(), device.device.version_code.clone()),
        ("version_name".to_string(), device.device.version_name.clone()),
        ("device_platform".to_string(), "android".to_string()),
        ("os".to_string(), "android".to_string()),
        ("ssmix".to_string(), "a".to_string()),
        ("device_type".to_string(), device.device.device_type.clone()),
        ("device_brand".to_string(), device.device.device_brand.clone()),
        ("language".to_string(), "zh".to_string()),
        ("os_api".to_string(), device.device.os_api.clone()),
        ("os_version".to_string(), device.device.os_version.clone()),
        (
            "manifest_version_code".to_string(),
            device.device.version_code.clone(),
        ),
        ("resolution".to_string(), device.device.resolution.clone()),
        ("dpi".to_string(), device.device.dpi.clone()),
        (
            "update_version_code".to_string(),
            device.device.update_version_code.clone(),
        ),
        ("_rticket".to_string(), now.to_string()),
        ("host_abi".to_string(), device.device.host_abi.clone()),
        ("dragon_device_type".to_string(), "phone".to_string()),
        ("pv_player".to_string(), device.device.version_code.clone()),
        ("compliance_status".to_string(), "0".to_string()),
        ("need_personal_recommend".to_string(), "1".to_string()),
        ("player_so_load".to_string(), "1".to_string()),
        ("is_android_pad_screen".to_string(), "0".to_string()),
        ("rom_version".to_string(), device.device.rom_version.clone()),
        ("cdid".to_string(), device.device.cdid.clone()),
    ]
}

fn build_url(base_url: &str, path: &str, params: &[(String, String)]) -> ServiceResult<String> {
    let raw = format!("{}{}", base_url.trim_end_matches('/'), path);
    let mut url = Url::parse(&raw)
        .map_err(|error| ServiceError::internal(format!("URL 构建失败 {raw}: {error}")))?;
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in params {
            pairs.append_pair(key, value);
        }
    }
    Ok(url.to_string())
}

fn parse_search_response(root: &Value, tab_type: u32) -> SearchResponse {
    let data = root.get("data").unwrap_or(&Value::Null);
    let search_tabs = first_array(&[
        root.get("search_tabs"),
        data.get("search_tabs"),
        root.get("searchTabs"),
        data.get("searchTabs"),
    ]);

    let mut response = SearchResponse::default();

    if let Some(tabs) = search_tabs {
        for tab in tabs {
            let current_tab_type = tab
                .get("tab_type")
                .and_then(Value::as_u64)
                .unwrap_or_default() as u32;
            if current_tab_type == tab_type {
                let books = parse_books_from_tab(tab);
                response.total = tab
                    .get("total")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize)
                    .unwrap_or(books.len());
                response.has_more = bool_from_value(tab.get("has_more")).unwrap_or(false);
                response.search_id = first_non_blank_opt(&[
                    search_id_of(tab),
                    search_id_of(data),
                    search_id_of(root),
                ]);
                response.books = books;
                return response;
            }
        }

        for tab in tabs {
            let books = parse_books_from_tab(tab);
            if !books.is_empty() {
                response.total = tab
                    .get("total")
                    .and_then(Value::as_u64)
                    .map(|value| value as usize)
                    .unwrap_or(books.len());
                response.has_more = bool_from_value(tab.get("has_more")).unwrap_or(false);
                response.search_id = first_non_blank_opt(&[
                    search_id_of(tab),
                    search_id_of(data),
                    search_id_of(root),
                ]);
                response.books = books;
                return response;
            }
        }
    }

    if let Some(books_node) = first_array(&[data.get("books"), root.get("books")]) {
        response.books = parse_book_array(books_node);
        response.total = data
            .get("total")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(response.books.len());
        response.has_more = bool_from_value(data.get("has_more")).unwrap_or(false);
    }
    response.search_id = first_non_blank_opt(&[search_id_of(data), deep_find_search_id(root)]);
    response
}

fn parse_books_from_tab(tab: &Value) -> Vec<BookItem> {
    let mut books = Vec::new();
    if let Some(items) = tab.get("data").and_then(Value::as_array) {
        for item in items {
            if let Some(book_data) = item.get("book_data").and_then(Value::as_array) {
                books.extend(parse_book_array(book_data));
            }
        }
    }
    if books.is_empty() {
        if let Some(direct) = tab.get("books").and_then(Value::as_array) {
            books.extend(parse_book_array(direct));
        }
    }
    books
}

fn parse_book_array(values: &[Value]) -> Vec<BookItem> {
    values.iter().map(parse_book_item).collect()
}

fn parse_book_item(value: &Value) -> BookItem {
    BookItem {
        book_id: string_field(value, "book_id"),
        book_name: string_field(value, "book_name"),
        author: string_field(value, "author"),
        description: first_non_blank(&[
            value.get("abstract").and_then(Value::as_str).unwrap_or_default(),
            value.get("book_abstract_v2")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        ]),
        cover_url: first_non_blank(&[
            value.get("thumb_url").and_then(Value::as_str).unwrap_or_default(),
            value.get("detail_page_thumb_url")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        ]),
        last_chapter_title: string_field(value, "last_chapter_title"),
        category: string_field(value, "category"),
        word_count: value
            .get("word_number")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
    }
}

fn trim_directory_for_minimal(directory: &mut DirectoryResponse) {
    let minimal_items: Vec<DirectoryItemData> = directory
        .item_data_list
        .iter()
        .filter_map(|item| {
            if item.item_id.trim().is_empty() {
                None
            } else {
                Some(DirectoryItemData {
                    item_id: item.item_id.clone(),
                    title: item.title.clone(),
                    chapter_index: None,
                    is_latest: None,
                    first_pass_time: None,
                    first_pass_time_str: None,
                    sort_order: None,
                    is_free: None,
                })
            }
        })
        .collect();
    directory.item_data_list = minimal_items;
    directory.serial_count = Some(
        directory
            .serial_count
            .unwrap_or(directory.item_data_list.len() as i32)
            .max(directory.item_data_list.len() as i32),
    );
    directory.book_info = None;
    directory.catalog_data = None;
    directory.field_cache_status = None;
    directory.ban_recover = None;
    directory.additional_item_data_list = None;
}

fn build_book_info(book_id: &str, directory: &DirectoryResponse) -> ServiceResult<BookInfo> {
    let info: &UpstreamBookInfo = directory
        .book_info
        .as_ref()
        .ok_or_else(|| ServiceError::internal("书籍信息不存在"))?;

    let description = first_non_blank(&[
        info.abstract_content.as_deref().unwrap_or_default(),
        info.book_abstract_v2.as_deref().unwrap_or_default(),
    ]);
    let total_chapters = info
        .serial_count
        .or(directory.serial_count)
        .unwrap_or_else(|| directory.item_data_list.len() as i32);

    Ok(BookInfo {
        book_id: book_id.to_string(),
        book_name: info.book_name.clone().unwrap_or_default(),
        author: info.author.clone().unwrap_or_default(),
        description,
        cover_url: info.thumb_url.clone().unwrap_or_default(),
        total_chapters,
        word_number: info.word_number.unwrap_or_default(),
        last_chapter_title: info.last_chapter_title.clone().unwrap_or_default(),
        category: info.category.clone().unwrap_or_default(),
        status: info.status.unwrap_or_default(),
    })
}

fn chapter_context(directory: &DirectoryResponse, chapter_id: &str) -> Option<ChapterContext> {
    let index = directory
        .item_data_list
        .iter()
        .position(|item| item.item_id == chapter_id)?;
    let prev = if index > 0 {
        Some(directory.item_data_list[index - 1].item_id.clone())
    } else {
        None
    };
    let next = directory
        .item_data_list
        .get(index + 1)
        .map(|item| item.item_id.clone());
    let title = directory.item_data_list.get(index).map(|item| item.title.clone());
    Some(ChapterContext {
        chapter_index: (index + 1) as i32,
        prev_chapter_id: prev,
        next_chapter_id: next,
        is_free: index < 5,
        title,
    })
}

fn normalize_install_id(cookie: &str, install_id: &str) -> String {
    if install_id.trim().is_empty() {
        return cookie.to_string();
    }
    let key = "install_id=";
    if let Some(position) = cookie.to_ascii_lowercase().find(key) {
        let value_start = position + key.len();
        let value_end = cookie[value_start..]
            .find(';')
            .map(|offset| value_start + offset)
            .unwrap_or(cookie.len());
        let mut output = String::new();
        output.push_str(&cookie[..value_start]);
        output.push_str(install_id);
        output.push_str(&cookie[value_end..]);
        output
    } else if cookie.trim().is_empty() {
        format!("{key}{install_id}")
    } else if cookie.trim().ends_with(';') {
        format!("{} {key}{install_id};", cookie.trim())
    } else {
        format!("{}; {key}{install_id};", cookie.trim())
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn bool01(value: bool) -> String {
    if value {
        "1".to_string()
    } else {
        "0".to_string()
    }
}

fn bounded_delay(min_ms: u64, max_ms: u64) -> u64 {
    let min = min_ms.min(max_ms);
    let max = max_ms.max(min_ms);
    if min == max {
        min
    } else {
        rand::thread_rng().gen_range(min..=max)
    }
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn bool_from_value(value: Option<&Value>) -> Option<bool> {
    match value {
        Some(Value::Bool(flag)) => Some(*flag),
        Some(Value::Number(number)) => number.as_i64().map(|value| value != 0),
        Some(Value::String(text)) => match text.trim() {
            "1" | "true" | "TRUE" => Some(true),
            "0" | "false" | "FALSE" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn first_array<'a>(values: &[Option<&'a Value>]) -> Option<&'a [Value]> {
    for value in values {
        if let Some(inner) = *value {
            if let Some(items) = inner.as_array() {
                return Some(items.as_slice());
            }
        }
    }
    None
}

fn search_id_of(value: &Value) -> Option<String> {
    first_non_blank_opt(&[
        value.get("search_id")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        value.get("searchId")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        value.get("search_id_str")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    ])
}

fn deep_find_search_id(root: &Value) -> Option<String> {
    if let Some(value) = search_id_of(root) {
        return Some(value);
    }
    match root {
        Value::Array(items) => items.iter().find_map(deep_find_search_id),
        Value::Object(map) => map.values().find_map(deep_find_search_id),
        _ => None,
    }
}

fn first_non_blank(values: &[&str]) -> String {
    values
        .iter()
        .find_map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .unwrap_or_default()
}

fn first_non_blank_opt(values: &[Option<String>]) -> Option<String> {
    values.iter().flatten().find_map(|value| {
        if value.trim().is_empty() {
            None
        } else {
            Some(value.clone())
        }
    })
}

#[derive(Debug, Clone)]
struct SearchUpstreamRequest {
    query: String,
    offset: usize,
    count: usize,
    passback: usize,
    tab_type: u32,
    search_id: Option<String>,
    bookshelf_search_plan: i32,
    from_rs: bool,
    user_is_login: i32,
    bookstore_tab: i32,
    search_source: i32,
    clicked_content: String,
    search_source_id: String,
    use_lynx: bool,
    use_correct: bool,
    tab_name: String,
    is_first_enter_search: bool,
    client_ab_info: String,
    last_search_page_interval: i32,
    line_words_num: i32,
    last_consume_interval: i32,
    pad_column_cover: i32,
    normal_session_id: String,
    cold_start_session_id: String,
    charging: i32,
    screen_brightness: i32,
    battery_pct: i32,
    down_speed: i32,
    sys_dark_mode: i32,
    app_dark_mode: i32,
    font_scale: i32,
    is_android_pad_screen: i32,
    network_type: i32,
    current_volume: i32,
}

impl SearchUpstreamRequest {
    fn new(
        query: String,
        offset: usize,
        count: usize,
        tab_type: u32,
        search_id: Option<String>,
        _device: &UpstreamDevice,
    ) -> Self {
        Self {
            query,
            offset,
            count,
            passback: offset,
            tab_type,
            search_id,
            bookshelf_search_plan: 4,
            from_rs: false,
            user_is_login: 0,
            bookstore_tab: 2,
            search_source: 1,
            clicked_content: "search_history".to_string(),
            search_source_id: "his###".to_string(),
            use_lynx: false,
            use_correct: true,
            tab_name: "store".to_string(),
            is_first_enter_search: true,
            client_ab_info: "{}".to_string(),
            last_search_page_interval: 0,
            line_words_num: 0,
            last_consume_interval: 0,
            pad_column_cover: 0,
            normal_session_id: Uuid::new_v4().to_string(),
            cold_start_session_id: Uuid::new_v4().to_string(),
            charging: 1,
            screen_brightness: 72,
            battery_pct: 78,
            down_speed: 89_121,
            sys_dark_mode: 0,
            app_dark_mode: 0,
            font_scale: 100,
            is_android_pad_screen: 0,
            network_type: 4,
            current_volume: 75,
        }
    }
}

#[derive(Debug, Clone)]
struct ChapterContext {
    chapter_index: i32,
    prev_chapter_id: Option<String>,
    next_chapter_id: Option<String>,
    is_free: bool,
    title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct BatchFullResponse {
    code: i64,
    message: String,
    #[serde(default)]
    data: HashMap<String, ItemContent>,
}

#[derive(Debug, Clone, Deserialize)]
struct ItemContent {
    code: Option<i64>,
    title: Option<String>,
    content: Option<String>,
    #[serde(rename = "novel_data")]
    novel_data: Option<NovelData>,
    #[serde(rename = "key_version")]
    key_version: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct NovelData {
    author: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_search_tabs() {
        let root: Value = serde_json::from_str(
            r#"{
              "code": 0,
              "data": {
                "search_tabs": [
                  {
                    "tab_type": 3,
                    "total": 1,
                    "has_more": false,
                    "search_id": "sid-1",
                    "data": [
                      {
                        "book_data": [
                          {
                            "book_id": "100",
                            "book_name": "测试书",
                            "author": "作者",
                            "abstract": "简介",
                            "thumb_url": "https://example.com/1.jpg",
                            "last_chapter_title": "最新章",
                            "category": "分类",
                            "word_number": 123
                          }
                        ]
                      }
                    ]
                  }
                ]
              }
            }"#,
        )
        .unwrap();

        let parsed = parse_search_response(&root, 3);
        assert_eq!(parsed.total, 1);
        assert_eq!(parsed.search_id.as_deref(), Some("sid-1"));
        assert_eq!(parsed.books[0].book_name, "测试书");
    }

    #[test]
    fn normalizes_install_id() {
        let cookie = "store-region=cn-zj; store-region-src=did";
        assert_eq!(
            normalize_install_id(cookie, "123"),
            "store-region=cn-zj; store-region-src=did; install_id=123;"
        );
    }
}
