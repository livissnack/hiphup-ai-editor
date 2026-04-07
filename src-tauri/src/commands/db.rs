use redb::Database;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::Manager;

static STATE_DB: OnceLock<Database> = OnceLock::new();

fn state_db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("state.redb"))
}

pub fn open_state_db(app: &tauri::AppHandle) -> Result<&'static Database, String> {
    if let Some(db) = STATE_DB.get() {
        return Ok(db);
    }

    let path = state_db_path(app)?;
    let db = Database::create(path).map_err(|e| e.to_string())?;
    let _ = STATE_DB.set(db);

    STATE_DB
        .get()
        .ok_or_else(|| "Failed to initialize state database.".to_string())
}
