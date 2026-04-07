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

fn load_ai_config(app: &tauri::AppHandle) -> Result<(String, String), String> {
    let db = open_state_db(app)?;
    let read_txn = db.begin_read().map_err(|e| e.to_string())?;
    let table = read_txn.open_table(APP_KV).map_err(|e| e.to_string())?;

    let base_url = table
        .get("ai_base_url")
        .map_err(|e| e.to_string())?
        .map(|v| v.value().to_string())
        .unwrap_or_else(|| "https://aihubmix.com/v1".to_string());
    let api_key = table
        .get("ai_api_key")
        .map_err(|e| e.to_string())?
        .map(|v| v.value().to_string())
        .unwrap_or_default();

    Ok((base_url, api_key))
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
    model.trim().to_ascii_lowercase().ends_with("-free")
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
        return Err(if body.trim().is_empty() {
            format!("Upstream error: HTTP {status}")
        } else {
            format!("Upstream error: HTTP {status} - {body}")
        });
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
        return Err(if body.trim().is_empty() {
            format!("Upstream error: HTTP {status}")
        } else {
            format!("Upstream error: HTTP {status} - {body}")
        });
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
    match call_chat_once(
        &client,
        &endpoint,
        &api_key,
        &preferred_model,
        &payload.messages,
        temperature,
        stream,
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
        match call_chat_once(
            &client,
            &endpoint,
            &api_key,
            &free_model,
            &payload.messages,
            temperature,
            stream,
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
    match call_chat_once_stream(
        &app,
        &request_id,
        &client,
        &endpoint,
        &api_key,
        &preferred_model,
        &payload.messages,
        temperature,
        stream,
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
        match call_chat_once_stream(
            &app,
            &request_id,
            &client,
            &endpoint,
            &api_key,
            &free_model,
            &payload.messages,
            temperature,
            stream,
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
