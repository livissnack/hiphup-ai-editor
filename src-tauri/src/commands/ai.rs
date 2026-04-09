use crate::commands::db::open_state_db;
use futures_util::StreamExt;
use redb::{ReadableDatabase, TableDefinition};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Emitter;

#[derive(Deserialize)]
pub struct ChatCompletionRequest {
    model: String,
    messages: Value,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    stream: Option<bool>,
}

#[derive(Serialize)]
struct UpstreamChatRequest<'a> {
    model: &'a str,
    messages: &'a Value,
    temperature: f32,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: Option<Value>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: Option<ChatChoiceMessage>,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Option<Vec<ChatChoice>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResult {
    content: String,
    model: String,
    switched: bool,
    fallback_failures: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StreamChunkEvent {
    request_id: String,
    chunk: String,
}

static APP_KV: TableDefinition<&str, &str> = TableDefinition::new("app_kv");
const AI_FREE_RR_INDEX_KEY: &str = "ai_free_rr_index";
const AI_FREE_COOLDOWN_KEY: &str = "ai_free_cooldown_json";
const AI_FREE_COOLDOWN_MS: u64 = 5 * 60 * 1000;
const AI_FREE_FALLBACK_LIMIT: usize = 3;
const AI_FREE_429_RETRY_MAX: usize = 2;

fn load_ai_config(app: &tauri::AppHandle) -> Result<(String, String), String> {
    let c = crate::commands::state::load_ai_config_data(app)?;
    Ok((c.base_url, c.api_key))
}

fn save_ai_model(app: &tauri::AppHandle, model: &str) -> Result<(), String> {
    let db = open_state_db(app)?;
    let write_txn = db.begin_write().map_err(|e| e.to_string())?;
    {
        let mut table = write_txn.open_table(APP_KV).map_err(|e| e.to_string())?;
        table.insert("ai_model", model).map_err(|e| e.to_string())?;
    }
    write_txn.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_message_content(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => {
            let mut parts: Vec<String> = Vec::new();
            for item in items {
                let t = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if t == "text" {
                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                        parts.push(text.to_string());
                    }
                }
            }
            parts.join("\n")
        }
        _ => String::new(),
    }
}

fn root_from_base_url(base_url: &str) -> String {
    base_url
        .trim_end_matches('/')
        .trim_end_matches("/v1")
        .to_string()
}

fn is_free_model_id(model: &str) -> bool {
    let id = model.trim().to_ascii_lowercase();
    id.ends_with("-free") || id.ends_with(":free") || id.contains(":free:")
}

fn is_http_429_error(err: &str) -> bool {
    err.contains("HTTP 429")
}

fn format_upstream_http_error(status: reqwest::StatusCode, body: &str) -> String {
    let mut msg = if body.trim().is_empty() {
        format!("Upstream error: HTTP {status}")
    } else {
        format!("Upstream error: HTTP {status} - {body}")
    };
    if status.as_u16() == 401 {
        msg.push_str(
            " Hint: use the API key from the same provider as the active AI source (OpenRouter vs Aihubmix keys are not interchangeable).",
        );
    }
    msg
}

async fn call_chat_once(
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
    model: &str,
    messages: &Value,
    temperature: f32,
    stream: bool,
) -> Result<String, String> {
    let upstream_payload = UpstreamChatRequest {
        model,
        messages,
        temperature,
        stream,
    };

    let response = client
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&upstream_payload)
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format_upstream_http_error(status, &body));
    }

    let parsed: ChatCompletionResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid upstream response: {e}"))?;

    let content = parsed
        .choices
        .and_then(|mut v| v.drain(..).next())
        .and_then(|c| c.message)
        .and_then(|m| m.content)
        .map(|c| parse_message_content(&c))
        .unwrap_or_default();

    if content.trim().is_empty() {
        Ok("No response.".to_string())
    } else {
        Ok(content)
    }
}

async fn call_chat_with_429_retry(
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
    model: &str,
    messages: &Value,
    temperature: f32,
    stream: bool,
    allow_retry: bool,
) -> Result<String, String> {
    let max_retry = if allow_retry { AI_FREE_429_RETRY_MAX } else { 0 };
    let mut last_err = String::new();
    for attempt in 0..=max_retry {
        match call_chat_once(
            client,
            endpoint,
            api_key,
            model,
            messages,
            temperature,
            stream,
        )
        .await
        {
            Ok(content) => return Ok(content),
            Err(err) => {
                last_err = err;
                if attempt >= max_retry || !is_http_429_error(&last_err) {
                    break;
                }
            }
        }
    }
    Err(last_err)
}

async fn call_chat_once_stream(
    app: &tauri::AppHandle,
    request_id: &str,
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
    model: &str,
    messages: &Value,
    temperature: f32,
    stream: bool,
) -> Result<(String, bool), String> {
    let upstream_payload = UpstreamChatRequest {
        model,
        messages,
        temperature,
        stream,
    };

    let response = client
        .post(endpoint)
        .bearer_auth(api_key)
        .header("Accept", "text/event-stream")
        .json(&upstream_payload)
        .timeout(Duration::from_secs(45))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format_upstream_http_error(status, &body));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut content = String::new();
    let mut emitted_any = false;
    let mut done = false;

    let extract_delta_text = |json: &Value| -> String {
        let choice = json
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first());
        let Some(choice) = choice else {
            return String::new();
        };
        if let Some(delta_content) = choice.get("delta").and_then(|d| d.get("content")) {
            match delta_content {
                Value::String(s) => return s.to_string(),
                Value::Array(arr) => {
                    let mut parts = Vec::<String>::new();
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            parts.push(s.to_string());
                            continue;
                        }
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            parts.push(text.to_string());
                        }
                    }
                    return parts.join("");
                }
                _ => {}
            }
        }
        if let Some(s) = choice.get("text").and_then(|v| v.as_str()) {
            return s.to_string();
        }
        String::new()
    };

    while let Some(next) = stream.next().await {
        let bytes = next.map_err(|e| format!("Stream read failed: {e}"))?;
        let piece = String::from_utf8_lossy(&bytes);
        // Normalize CRLF SSE frames so delimiter parsing is stable across proxies.
        buffer.push_str(&piece.replace("\r\n", "\n"));

        while let Some(frame_end) = buffer.find("\n\n") {
            let frame = buffer[..frame_end].to_string();
            buffer = buffer[frame_end + 2..].to_string();
            let mut data_lines: Vec<String> = Vec::new();
            for raw_line in frame.lines() {
                let line = raw_line.trim_end_matches('\r').trim();
                if line.is_empty() || line.starts_with(':') || line.starts_with("event:") {
                    continue;
                }
                if let Some(rest) = line.strip_prefix("data:") {
                    data_lines.push(rest.trim().to_string());
                }
            }
            if data_lines.is_empty() {
                continue;
            }
            let data = data_lines.join("\n");
            if data.trim() == "[DONE]" {
                done = true;
                break;
            }
            let Ok(json) = serde_json::from_str::<Value>(&data) else {
                continue;
            };
            let chunk = extract_delta_text(&json);
            if chunk.is_empty() {
                continue;
            }
            content.push_str(&chunk);
            emitted_any = true;
            let _ = app.emit(
                "aihub_chat_chunk",
                StreamChunkEvent {
                    request_id: request_id.to_string(),
                    chunk,
                },
            );
        }
        if done {
            break;
        }
    }

    if content.trim().is_empty() {
        Ok(("No response.".to_string(), emitted_any))
    } else {
        Ok((content, emitted_any))
    }
}

async fn call_chat_stream_with_429_retry(
    app: &tauri::AppHandle,
    request_id: &str,
    client: &reqwest::Client,
    endpoint: &str,
    api_key: &str,
    model: &str,
    messages: &Value,
    temperature: f32,
    stream: bool,
    allow_retry: bool,
) -> Result<(String, bool), String> {
    let max_retry = if allow_retry { AI_FREE_429_RETRY_MAX } else { 0 };
    let mut last_err = String::new();
    for attempt in 0..=max_retry {
        match call_chat_once_stream(
            app,
            request_id,
            client,
            endpoint,
            api_key,
            model,
            messages,
            temperature,
            stream,
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(err) => {
                last_err = err;
                if attempt >= max_retry || !is_http_429_error(&last_err) {
                    break;
                }
            }
        }
    }
    Err(last_err)
}

async fn load_free_models(
    client: &reqwest::Client,
    root: &str,
    api_key: &str,
) -> Vec<String> {
    let endpoint = format!("{root}/api/v1/models");
    let response = client
        .get(endpoint)
        .bearer_auth(api_key)
        .timeout(Duration::from_secs(8))
        .send()
        .await;

    let Ok(resp) = response else {
        return vec![];
    };
    if !resp.status().is_success() {
        return vec![];
    }
    let Ok(json) = resp.json::<Value>().await else {
        return vec![];
    };
    let Some(items) = json.get("data").and_then(|v| v.as_array()) else {
        return vec![];
    };
    items
        .iter()
        .filter_map(|item| item.get("model_id").and_then(|v| v.as_str()))
        .map(|s| s.trim().to_string())
        .filter(|id| !id.is_empty() && id.to_ascii_lowercase().ends_with("-free"))
        .collect()
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn load_free_rr_index(app: &tauri::AppHandle) -> Result<usize, String> {
    let db = open_state_db(app)?;
    let read_txn = db.begin_read().map_err(|e| e.to_string())?;
    let table = read_txn.open_table(APP_KV).map_err(|e| e.to_string())?;
    let idx = table
        .get(AI_FREE_RR_INDEX_KEY)
        .map_err(|e| e.to_string())?
        .and_then(|v| v.value().parse::<usize>().ok())
        .unwrap_or(0);
    Ok(idx)
}

fn save_free_rr_index(app: &tauri::AppHandle, idx: usize) -> Result<(), String> {
    let db = open_state_db(app)?;
    let write_txn = db.begin_write().map_err(|e| e.to_string())?;
    {
        let mut table = write_txn.open_table(APP_KV).map_err(|e| e.to_string())?;
        let text = idx.to_string();
        table
            .insert(AI_FREE_RR_INDEX_KEY, text.as_str())
            .map_err(|e| e.to_string())?;
    }
    write_txn.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn load_free_cooldown(app: &tauri::AppHandle) -> Result<HashMap<String, u64>, String> {
    let db = open_state_db(app)?;
    let read_txn = db.begin_read().map_err(|e| e.to_string())?;
    let table = read_txn.open_table(APP_KV).map_err(|e| e.to_string())?;
    let raw = table
        .get(AI_FREE_COOLDOWN_KEY)
        .map_err(|e| e.to_string())?
        .map(|v| v.value().to_string())
        .unwrap_or_else(|| "{}".to_string());
    serde_json::from_str::<HashMap<String, u64>>(&raw).map_err(|e| e.to_string())
}

fn save_free_cooldown(app: &tauri::AppHandle, cooldown: &HashMap<String, u64>) -> Result<(), String> {
    let db = open_state_db(app)?;
    let write_txn = db.begin_write().map_err(|e| e.to_string())?;
    {
        let mut table = write_txn.open_table(APP_KV).map_err(|e| e.to_string())?;
        let text = serde_json::to_string(cooldown).map_err(|e| e.to_string())?;
        table
            .insert(AI_FREE_COOLDOWN_KEY, text.as_str())
            .map_err(|e| e.to_string())?;
    }
    write_txn.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn free_fallback_candidates(
    free_models: &[String],
    preferred_model: &str,
    rr_index: usize,
    cooldown: &HashMap<String, u64>,
    now_ms: u64,
    limit: usize,
) -> Vec<String> {
    if free_models.is_empty() {
        return vec![];
    }
    let len = free_models.len();
    let mut ordered = Vec::with_capacity(len);
    for i in 0..len {
        ordered.push(free_models[(rr_index + i) % len].clone());
    }
    ordered
        .into_iter()
        .filter(|m| m != preferred_model)
        .filter(|m| cooldown.get(m).copied().unwrap_or(0) <= now_ms)
        .take(limit)
        .collect()
}

#[tauri::command]
pub async fn aihub_chat(
    app: tauri::AppHandle,
    payload: ChatCompletionRequest,
) -> Result<ChatResult, String> {
    let (base_url, api_key) = load_ai_config(&app)?;
    if api_key.trim().is_empty() {
        return Err("API key is required.".to_string());
    }

    let root = root_from_base_url(&base_url);
    let endpoint = format!("{root}/v1/chat/completions");
    let client = reqwest::Client::new();
    let temperature = payload.temperature.unwrap_or(0.2);
    let stream = payload.stream.unwrap_or(false);
    let preferred_model = payload.model.trim().to_string();

    let mut failed_reasons: Vec<String> = Vec::new();
    match call_chat_with_429_retry(
        &client,
        &endpoint,
        &api_key,
        &preferred_model,
        &payload.messages,
        temperature,
        stream,
        is_free_model_id(&preferred_model),
    )
    .await
    {
        Ok(content) => {
            return Ok(ChatResult {
                content,
                model: preferred_model,
                switched: false,
                fallback_failures: vec![],
            });
        }
        Err(err) => {
            failed_reasons.push(format!("{preferred_model}: {err}"));
        }
    }

    // Respect explicit paid-model selection: do not auto-switch to free models.
    if !is_free_model_id(&preferred_model) {
        return Err(format!(
            "Selected model failed and auto-fallback is disabled for paid models. Reason: {}",
            failed_reasons.join(" | ")
        ));
    }

    let free_models = load_free_models(&client, &root, &api_key).await;
    let rr_index = load_free_rr_index(&app).unwrap_or(0);
    let mut cooldown = load_free_cooldown(&app).unwrap_or_default();
    let now_ms = now_unix_ms();
    cooldown.retain(|_, expires_at| *expires_at > now_ms);
    let candidates = free_fallback_candidates(
        &free_models,
        &preferred_model,
        rr_index,
        &cooldown,
        now_ms,
        AI_FREE_FALLBACK_LIMIT,
    );
    let mut last_error = String::from("Primary model failed, and no free fallback model is available.");
    for free_model in candidates {
        match call_chat_with_429_retry(
            &client,
            &endpoint,
            &api_key,
            &free_model,
            &payload.messages,
            temperature,
            stream,
            true,
        )
        .await
        {
            Ok(content) => {
                let _ = save_ai_model(&app, &free_model);
                if let Some(pos) = free_models.iter().position(|m| m == &free_model) {
                    let _ = save_free_rr_index(&app, (pos + 1) % free_models.len().max(1));
                }
                cooldown.remove(&free_model);
                let _ = save_free_cooldown(&app, &cooldown);
                return Ok(ChatResult {
                    content,
                    model: free_model,
                    switched: true,
                    fallback_failures: failed_reasons.clone(),
                });
            }
            Err(err) => {
                last_error = err;
                failed_reasons.push(format!("{free_model}: {last_error}"));
                cooldown.insert(free_model.clone(), now_unix_ms() + AI_FREE_COOLDOWN_MS);
            }
        }
    }
    let _ = save_free_cooldown(&app, &cooldown);

    let reasons_text = if failed_reasons.is_empty() {
        "No fallback models were attempted (all skipped by filters/cooldown).".to_string()
    } else {
        failed_reasons.join(" | ")
    };
    Err(format!(
        "Primary model failed and all free fallbacks failed. Last error: {last_error}. Failed models: {reasons_text}"
    ))
}

#[tauri::command]
pub async fn aihub_chat_stream(
    app: tauri::AppHandle,
    payload: ChatCompletionRequest,
    request_id: String,
) -> Result<ChatResult, String> {
    let (base_url, api_key) = load_ai_config(&app)?;
    if api_key.trim().is_empty() {
        return Err("API key is required.".to_string());
    }

    let root = root_from_base_url(&base_url);
    let endpoint = format!("{root}/v1/chat/completions");
    let client = reqwest::Client::new();
    let temperature = payload.temperature.unwrap_or(0.2);
    let stream = payload.stream.unwrap_or(true);
    let preferred_model = payload.model.trim().to_string();

    let mut failed_reasons: Vec<String> = Vec::new();
    match call_chat_stream_with_429_retry(
        &app,
        &request_id,
        &client,
        &endpoint,
        &api_key,
        &preferred_model,
        &payload.messages,
        temperature,
        stream,
        is_free_model_id(&preferred_model),
    )
    .await
    {
        Ok((content, _)) => {
            return Ok(ChatResult {
                content,
                model: preferred_model,
                switched: false,
                fallback_failures: vec![],
            });
        }
        Err(err) => {
            failed_reasons.push(format!("{preferred_model}: {err}"));
        }
    }

    // Respect explicit paid-model selection: do not auto-switch to free models.
    if !is_free_model_id(&preferred_model) {
        return Err(format!(
            "Selected model failed and auto-fallback is disabled for paid models. Reason: {}",
            failed_reasons.join(" | ")
        ));
    }

    let free_models = load_free_models(&client, &root, &api_key).await;
    let rr_index = load_free_rr_index(&app).unwrap_or(0);
    let mut cooldown = load_free_cooldown(&app).unwrap_or_default();
    let now_ms = now_unix_ms();
    cooldown.retain(|_, expires_at| *expires_at > now_ms);
    let candidates = free_fallback_candidates(
        &free_models,
        &preferred_model,
        rr_index,
        &cooldown,
        now_ms,
        AI_FREE_FALLBACK_LIMIT,
    );
    let mut last_error = String::from("Primary model failed, and no free fallback model is available.");
    for free_model in candidates {
        match call_chat_stream_with_429_retry(
            &app,
            &request_id,
            &client,
            &endpoint,
            &api_key,
            &free_model,
            &payload.messages,
            temperature,
            stream,
            true,
        )
        .await
        {
            Ok((content, _emitted_any)) => {
                let _ = save_ai_model(&app, &free_model);
                if let Some(pos) = free_models.iter().position(|m| m == &free_model) {
                    let _ = save_free_rr_index(&app, (pos + 1) % free_models.len().max(1));
                }
                cooldown.remove(&free_model);
                let _ = save_free_cooldown(&app, &cooldown);
                return Ok(ChatResult {
                    content,
                    model: free_model,
                    switched: true,
                    fallback_failures: failed_reasons.clone(),
                });
            }
            Err(err) => {
                last_error = err;
                failed_reasons.push(format!("{free_model}: {last_error}"));
                cooldown.insert(free_model.clone(), now_unix_ms() + AI_FREE_COOLDOWN_MS);
            }
        }
    }
    let _ = save_free_cooldown(&app, &cooldown);

    let reasons_text = if failed_reasons.is_empty() {
        "No fallback models were attempted (all skipped by filters/cooldown).".to_string()
    } else {
        failed_reasons.join(" | ")
    };
    Err(format!(
        "Primary model failed and all free fallbacks failed. Last error: {last_error}. Failed models: {reasons_text}"
    ))
}

#[tauri::command]
pub async fn aihub_connection_status(app: tauri::AppHandle) -> Result<bool, String> {
    let (base_url, api_key) = load_ai_config(&app)?;
    if api_key.trim().is_empty() {
        return Ok(false);
    }

    let root = base_url.trim_end_matches('/').to_string();
    let endpoint = format!("{root}/models");

    let client = reqwest::Client::new();
    let response = client
        .get(endpoint)
        .bearer_auth(api_key)
        .timeout(Duration::from_secs(8))
        .send()
        .await;

    match response {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(_) => Ok(false),
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OpenRouterModelItem {
    pub id: String,
    pub is_free: bool,
    pub supports_image: bool,
    pub supports_audio: bool,
    pub input_modalities: Vec<String>,
    pub output_modalities: Vec<String>,
}

fn parse_modalities(value: Option<&Value>) -> Vec<String> {
    let Some(v) = value else {
        return vec![];
    };
    if let Some(arr) = v.as_array() {
        return arr
            .iter()
            .filter_map(|x| x.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    if let Some(s) = v.as_str() {
        return s
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();
    }
    vec![]
}

fn parse_pricing_number(value: Option<&Value>) -> Option<f64> {
    let Some(v) = value else {
        return None;
    };
    if let Some(n) = v.as_f64() {
        return Some(n);
    }
    if let Some(s) = v.as_str() {
        return s.trim().parse::<f64>().ok();
    }
    None
}

fn has_modality(modalities: &[String], target: &str) -> bool {
    modalities.iter().any(|m| m.eq_ignore_ascii_case(target))
}

fn is_chat_model(
    input_modalities: &[String],
    output_modalities: &[String],
    modality_text: Option<&str>,
    model_type: Option<&str>,
) -> bool {
    if let Some(t) = model_type {
        let t = t.trim().to_ascii_lowercase();
        if t == "llm" || t == "chat" || t == "text" {
            return true;
        }
        if t == "video" || t == "image" || t == "audio" || t == "rerank" || t == "embedding" {
            return false;
        }
    }
    // Chat-completions compatible models should at least accept text input and produce text output.
    if has_modality(input_modalities, "text") && has_modality(output_modalities, "text") {
        return true;
    }
    // Some providers only expose input modalities for text models.
    if has_modality(input_modalities, "text") && output_modalities.is_empty() {
        return true;
    }
    // Fallback for providers exposing only architecture.modality string.
    modality_text
        .map(|s| {
            let v = s.to_ascii_lowercase();
            v.contains("->text") && v.contains("text")
        })
        .unwrap_or(false)
}

#[tauri::command]
pub async fn openrouter_list_models(
    models_url: Option<String>,
    api_key: Option<String>,
) -> Result<Vec<OpenRouterModelItem>, String> {
    let endpoint = models_url
        .unwrap_or_else(|| "https://openrouter.ai/api/v1/models?output_modalities=all".to_string());
    let client = reqwest::Client::new();
    let mut req = client.get(endpoint).timeout(Duration::from_secs(12));
    if let Some(k) = api_key.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        req = req.bearer_auth(k);
    }
    let resp = req.send().await.map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(if body.trim().is_empty() {
            format!("OpenRouter error: HTTP {status}")
        } else {
            format!("OpenRouter error: HTTP {status} - {body}")
        });
    }

    let json = resp
        .json::<Value>()
        .await
        .map_err(|e| format!("Invalid OpenRouter response: {e}"))?;
    let Some(items) = json.get("data").and_then(|v| v.as_array()) else {
        return Err("Invalid OpenRouter response: missing data[]".to_string());
    };

    let mut out: Vec<OpenRouterModelItem> = Vec::new();
    for item in items {
        let id = item
            .get("id")
            .or_else(|| item.get("model_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if id.is_empty() {
            continue;
        }

        // OpenRouter modalities live under `architecture` (preferred), but keep a top-level fallback for robustness.
        let arch = item.get("architecture");
        let input_modalities = {
            let from_arch = parse_modalities(arch.and_then(|a| a.get("input_modalities")));
            if !from_arch.is_empty() {
                from_arch
            } else {
                parse_modalities(item.get("input_modalities"))
            }
        };
        let mut output_modalities = {
            let from_arch = parse_modalities(arch.and_then(|a| a.get("output_modalities")));
            if !from_arch.is_empty() {
                from_arch
            } else {
                parse_modalities(item.get("output_modalities"))
            }
        };
        let model_type = item.get("types").and_then(|v| v.as_str());
        if output_modalities.is_empty() && matches!(model_type, Some("llm" | "chat" | "text")) {
            output_modalities.push("text".to_string());
        }

        let modality_text = arch.and_then(|a| a.get("modality")).and_then(|v| v.as_str());
        let supports_image = has_modality(&input_modalities, "image")
            || modality_text
                .map(|s| s.to_ascii_lowercase().contains("image"))
                .unwrap_or(false);

        let supports_audio = has_modality(&input_modalities, "audio")
            || has_modality(&output_modalities, "audio")
            || modality_text
                .map(|s| s.to_ascii_lowercase().contains("audio"))
                .unwrap_or(false);

        if !is_chat_model(&input_modalities, &output_modalities, modality_text, model_type) {
            continue;
        }

        // Strict free detection: only treat as free when both prompt and completion are exactly zero.
        let pricing = item.get("pricing");
        let prompt_price = parse_pricing_number(
            pricing
                .and_then(|p| p.get("prompt"))
                .or_else(|| pricing.and_then(|p| p.get("input"))),
        );
        let completion_price = parse_pricing_number(
            pricing
                .and_then(|p| p.get("completion"))
                .or_else(|| pricing.and_then(|p| p.get("output"))),
        );
        let is_free = match (prompt_price, completion_price) {
            (Some(p), Some(c)) => p <= 0.0 && c <= 0.0,
            _ => false,
        };

        out.push(OpenRouterModelItem {
            id,
            is_free,
            supports_image,
            supports_audio,
            input_modalities,
            output_modalities,
        });
    }

    out.sort_by(|a, b| {
        if a.is_free != b.is_free {
            return if a.is_free { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater };
        }
        a.id.cmp(&b.id)
    });
    Ok(out)
}
