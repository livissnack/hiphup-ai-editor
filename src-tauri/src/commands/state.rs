use crate::commands::db::open_state_db;
use redb::{ReadableDatabase, TableDefinition};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub workspace_path: String,
    pub open_tabs: Vec<String>,
    pub active_file_path: Option<String>,
    pub project_width: Option<u32>,
    pub ai_width: Option<u32>,
    pub show_project: Option<bool>,
    pub show_ai: Option<bool>,
    pub show_bottom: Option<bool>,
    #[serde(default)]
    pub tree_font_size: Option<u32>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    #[serde(default)]
    pub active_source_id: String,
    #[serde(default)]
    pub sources: Vec<AiSourceConfig>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AiSourceConfig {
    pub id: String,
    pub name: String,
    pub models_url: String,
    pub chat_base_url: String,
    /// Per-provider key; when non-empty, used instead of the global `ai_api_key` for this source.
    #[serde(default)]
    pub api_key: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AiChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatSession {
    pub id: String,
    pub title: String,
    pub model: String,
    pub messages: Vec<AiChatMessage>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub pin_order: i64,
    #[serde(default)]
    pub draft_input: String,
    #[serde(default)]
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatState {
    pub active_session_id: String,
    pub sessions: Vec<AiChatSession>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyAiChatState {
    pub model: String,
    pub messages: Vec<AiChatMessage>,
    #[serde(default)]
    pub draft_input: String,
}

static APP_KV: TableDefinition<&str, &str> = TableDefinition::new("app_kv");

#[tauri::command]
pub fn load_app_state(app: tauri::AppHandle) -> Result<Option<AppState>, String> {
    let db = open_state_db(&app)?;
    let read_txn = db.begin_read().map_err(|e| e.to_string())?;
    let table = read_txn.open_table(APP_KV).map_err(|e| e.to_string())?;
    let value = table.get("app_state").map_err(|e| e.to_string())?;
    let Some(value) = value else {
        return Ok(None);
    };
    let json: String = value.value().to_string();
    let state: AppState = serde_json::from_str(json.as_str()).map_err(|e| e.to_string())?;
    Ok(Some(state))
}

#[tauri::command]
pub fn save_app_state(app: tauri::AppHandle, state: AppState) -> Result<(), String> {
    let db = open_state_db(&app)?;
    let json = serde_json::to_string(&state).map_err(|e| e.to_string())?;
    let write_txn = db.begin_write().map_err(|e| e.to_string())?;
    {
        let mut table = write_txn.open_table(APP_KV).map_err(|e| e.to_string())?;
        table.insert("app_state", json.as_str()).map_err(|e| e.to_string())?;
    }
    write_txn.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn read_ai_config_merged(app: &tauri::AppHandle) -> Result<AiConfig, String> {
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
    let model = table
        .get("ai_model")
        .map_err(|e| e.to_string())?
        .map(|v| v.value().to_string())
        .unwrap_or_else(|| "gpt-4o-mini".to_string());
    let active_source_id = table
        .get("ai_active_source_id")
        .map_err(|e| e.to_string())?
        .map(|v| v.value().to_string())
        .unwrap_or_else(|| "openrouter".to_string());
    let sources = table
        .get("ai_sources")
        .map_err(|e| e.to_string())?
        .map(|v| v.value().to_string())
        .and_then(|s| serde_json::from_str::<Vec<AiSourceConfig>>(&s).ok())
        .unwrap_or_else(|| {
            vec![
                AiSourceConfig {
                    id: "openrouter".to_string(),
                    name: "OpenRouter".to_string(),
                    models_url: "https://openrouter.ai/api/v1/models?output_modalities=all"
                        .to_string(),
                    chat_base_url: "https://openrouter.ai/api/v1".to_string(),
                    api_key: String::new(),
                },
                AiSourceConfig {
                    id: "aihubmix".to_string(),
                    name: "Aihubmix".to_string(),
                    models_url: "https://aihubmix.com/api/v1/models".to_string(),
                    chat_base_url: "https://aihubmix.com/v1".to_string(),
                    api_key: String::new(),
                },
            ]
        });

    let mut config = AiConfig {
        base_url,
        api_key,
        model,
        active_source_id,
        sources,
    };
    if let Some(src) = config
        .sources
        .iter()
        .find(|s| s.id == config.active_source_id)
    {
        let k = src.api_key.trim();
        if !k.is_empty() {
            config.api_key = k.to_string();
        }
    }
    Ok(config)
}

#[tauri::command]
pub fn load_ai_config(app: tauri::AppHandle) -> Result<AiConfig, String> {
    read_ai_config_merged(&app)
}

/// For use from `commands::ai` (same merge rules as [load_ai_config]).
pub(crate) fn load_ai_config_data(app: &tauri::AppHandle) -> Result<AiConfig, String> {
    read_ai_config_merged(app)
}

#[tauri::command]
pub fn save_ai_config(app: tauri::AppHandle, config: AiConfig) -> Result<(), String> {
    let db = open_state_db(&app)?;
    let write_txn = db.begin_write().map_err(|e| e.to_string())?;
    {
        let mut table = write_txn.open_table(APP_KV).map_err(|e| e.to_string())?;
        table
            .insert("ai_base_url", config.base_url.as_str())
            .map_err(|e| e.to_string())?;
        table
            .insert("ai_api_key", config.api_key.as_str())
            .map_err(|e| e.to_string())?;
        table
            .insert("ai_model", config.model.as_str())
            .map_err(|e| e.to_string())?;
        table
            .insert("ai_active_source_id", config.active_source_id.as_str())
            .map_err(|e| e.to_string())?;
        let sources_json = serde_json::to_string(&config.sources).map_err(|e| e.to_string())?;
        table
            .insert("ai_sources", sources_json.as_str())
            .map_err(|e| e.to_string())?;
    }
    write_txn.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn load_ai_chat_state(app: tauri::AppHandle) -> Result<Option<AiChatState>, String> {
    let db = open_state_db(&app)?;
    let read_txn = db.begin_read().map_err(|e| e.to_string())?;
    let table = read_txn.open_table(APP_KV).map_err(|e| e.to_string())?;
    let Some(value) = table.get("ai_chat_state").map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let json: String = value.value().to_string();
    if let Ok(state) = serde_json::from_str::<AiChatState>(json.as_str()) {
        return Ok(Some(state));
    }
    if let Ok(legacy) = serde_json::from_str::<LegacyAiChatState>(json.as_str()) {
        let migrated = AiChatState {
            active_session_id: "default".to_string(),
            sessions: vec![AiChatSession {
                id: "default".to_string(),
                title: "Chat".to_string(),
                model: legacy.model,
                messages: legacy.messages,
                pinned: false,
                pin_order: 0,
                draft_input: legacy.draft_input,
                updated_at: 0,
            }],
        };
        return Ok(Some(migrated));
    }
    Err("Invalid ai chat state payload".to_string())
}

#[tauri::command]
pub fn save_ai_chat_state(app: tauri::AppHandle, state: AiChatState) -> Result<(), String> {
    let db = open_state_db(&app)?;
    let json = serde_json::to_string(&state).map_err(|e| e.to_string())?;
    let write_txn = db.begin_write().map_err(|e| e.to_string())?;
    {
        let mut table = write_txn.open_table(APP_KV).map_err(|e| e.to_string())?;
        table
            .insert("ai_chat_state", json.as_str())
            .map_err(|e| e.to_string())?;
    }
    write_txn.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn clear_ai_chat_state(app: tauri::AppHandle) -> Result<(), String> {
    let db = open_state_db(&app)?;
    let write_txn = db.begin_write().map_err(|e| e.to_string())?;
    {
        let mut table = write_txn.open_table(APP_KV).map_err(|e| e.to_string())?;
        let _ = table.remove("ai_chat_state").map_err(|e| e.to_string())?;
    }
    write_txn.commit().map_err(|e| e.to_string())?;
    Ok(())
}

