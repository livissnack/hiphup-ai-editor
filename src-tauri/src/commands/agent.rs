use crate::commands::db::open_state_db;
use crate::commands::state::AppState;
use redb::{ReadableDatabase, TableDefinition};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

static APP_KV: TableDefinition<&str, &str> = TableDefinition::new("app_kv");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStartRequest {
    pub goal: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentStep {
    pub index: usize,
    pub title: String,
    pub status: String,
    pub detail: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentRun {
    pub id: String,
    pub goal: String,
    pub status: String,
    pub steps: Vec<AgentStep>,
    pub summary: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSearchMatch {
    pub path: String,
    pub line: usize,
    pub text: String,
}

static AGENT_RUNS: OnceLock<Mutex<HashMap<String, AgentRun>>> = OnceLock::new();

fn runs_store() -> &'static Mutex<HashMap<String, AgentRun>> {
    AGENT_RUNS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn load_workspace_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let db = open_state_db(app)?;
    let read_txn = db.begin_read().map_err(|e| e.to_string())?;
    let table = read_txn.open_table(APP_KV).map_err(|e| e.to_string())?;
    let Some(value) = table.get("app_state").map_err(|e| e.to_string())? else {
        return Err("Open a workspace folder in the editor before using the Agent.".to_string());
    };
    let json: String = value.value().to_string();
    let state: AppState = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let wp = state.workspace_path.trim();
    if wp.is_empty() {
        return Err("Open a workspace folder in the editor before using the Agent.".to_string());
    }
    let p = PathBuf::from(wp);
    fs::canonicalize(&p).map_err(|e| format!("Workspace folder is not accessible: {e}"))
}

#[tauri::command]
pub fn agent_start_run(app: tauri::AppHandle, payload: AgentStartRequest) -> Result<AgentRun, String> {
    let run_id = format!(
        "run-{}-{}",
        chrono_like_now_ms(),
        random_suffix()
    );

    let run = AgentRun {
        id: run_id.clone(),
        goal: payload.goal.trim().to_string(),
        status: "running".to_string(),
        steps: vec![
            AgentStep {
                index: 1,
                title: "Analyze task".to_string(),
                status: "pending".to_string(),
                detail: "Waiting to start.".to_string(),
            },
            AgentStep {
                index: 2,
                title: "Scan workspace".to_string(),
                status: "pending".to_string(),
                detail: "Waiting to start.".to_string(),
            },
            AgentStep {
                index: 3,
                title: "Search codebase".to_string(),
                status: "pending".to_string(),
                detail: "Waiting to start.".to_string(),
            },
            AgentStep {
                index: 4,
                title: "Git working tree".to_string(),
                status: "pending".to_string(),
                detail: "Waiting to start.".to_string(),
            },
        ],
        summary: None,
    };

    let workspace_root = load_workspace_root(&app)?;
    let mut store = runs_store().lock().map_err(|e| e.to_string())?;
    store.insert(run_id, run.clone());

    let bg_run_id = run.id.clone();
    std::thread::spawn(move || {
        execute_run(bg_run_id, workspace_root);
    });

    Ok(run)
}

#[tauri::command]
pub fn agent_get_run(run_id: String) -> Result<Option<AgentRun>, String> {
    let store = runs_store().lock().map_err(|e| e.to_string())?;
    Ok(store.get(&run_id).cloned())
}

#[tauri::command]
pub fn agent_stop_run(run_id: String) -> Result<bool, String> {
    let mut store = runs_store().lock().map_err(|e| e.to_string())?;
    if let Some(run) = store.get_mut(&run_id) {
        if run.status == "running" {
            run.status = "stopped".to_string();
            run.summary = Some("Run stopped by user.".to_string());
        }
        return Ok(true);
    }
    Ok(false)
}

#[tauri::command]
pub fn agent_search_code(
    app: tauri::AppHandle,
    query: String,
    path: Option<String>,
) -> Result<Vec<AgentSearchMatch>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let workspace_root = load_workspace_root(&app)?;
    let target = path.unwrap_or_else(|| "src".to_string());
    let output = Command::new("rg")
        .args(["-n", "--no-heading", "--color", "never", q, &target])
        .current_dir(&workspace_root)
        .output()
        .map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                "ripgrep (rg) was not found. Install it and add it to your PATH (e.g. winget install BurntSushi.ripgrep.MSVC, cargo install ripgrep, or https://github.com/BurntSushi/ripgrep/releases)."
                    .to_string()
            } else {
                format!("Could not run ripgrep (rg): {e}")
            }
        })?;
    if !output.status.success() && output.stdout.is_empty() {
        return Ok(vec![]);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::<AgentSearchMatch>::new();
    let mut seen_paths = HashSet::<String>::new();
    for line in text.lines().take(80) {
        let mut parts = line.splitn(3, ':');
        let p = parts.next().unwrap_or("").trim();
        let ln = parts.next().unwrap_or("0").trim().parse::<usize>().unwrap_or(0);
        let body = parts.next().unwrap_or("").trim();
        if p.is_empty() || ln == 0 {
            continue;
        }
        let path = p.to_string();
        if !seen_paths.insert(path.clone()) {
            continue;
        }
        out.push(AgentSearchMatch {
            path,
            line: ln,
            text: body.to_string(),
        });
    }
    Ok(out)
}

#[tauri::command]
pub fn agent_read_file(app: tauri::AppHandle, path: String) -> Result<String, String> {
    let root = load_workspace_root(&app)?;
    let p = sanitize_workspace_path(&root, &path)?;
    let content = fs::read_to_string(&p).map_err(|e| format!("Read failed: {e}"))?;
    Ok(content)
}

#[tauri::command]
pub fn agent_write_file(app: tauri::AppHandle, path: String, content: String) -> Result<(), String> {
    let root = load_workspace_root(&app)?;
    let p = sanitize_workspace_path(&root, &path)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Create parent failed: {e}"))?;
    }
    fs::write(&p, content).map_err(|e| format!("Write failed: {e}"))?;
    Ok(())
}

fn chrono_like_now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn random_suffix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{:04x}", (nanos & 0xffff) as u16)
}

fn set_step(run_id: &str, step_idx: usize, status: &str, detail: String) {
    let mut store = match runs_store().lock() {
        Ok(s) => s,
        Err(_) => return,
    };
    let Some(run) = store.get_mut(run_id) else {
        return;
    };
    if run.status == "stopped" {
        return;
    }
    if let Some(step) = run.steps.get_mut(step_idx) {
        step.status = status.to_string();
        step.detail = detail;
    }
}

fn finish_run(run_id: &str, status: &str, summary: String) {
    let mut store = match runs_store().lock() {
        Ok(s) => s,
        Err(_) => return,
    };
    let Some(run) = store.get_mut(run_id) else {
        return;
    };
    if run.status == "stopped" {
        return;
    }
    run.status = status.to_string();
    run.summary = Some(summary);
}

fn is_stopped(run_id: &str) -> bool {
    let store = match runs_store().lock() {
        Ok(s) => s,
        Err(_) => return true,
    };
    let Some(run) = store.get(run_id) else {
        return true;
    };
    run.status == "stopped"
}

fn execute_run(run_id: String, workspace_root: PathBuf) {
    if is_stopped(&run_id) {
        return;
    }
    set_step(
        &run_id,
        0,
        "running",
        "Parsing goal and preparing execution context.".to_string(),
    );
    std::thread::sleep(std::time::Duration::from_millis(180));
    set_step(
        &run_id,
        0,
        "done",
        format!(
            "Goal accepted. Workspace root: {}.",
            workspace_root.display()
        ),
    );

    if is_stopped(&run_id) {
        return;
    }
    set_step(&run_id, 1, "running", "Scanning top-level workspace entries.".to_string());
    let scan_detail = match std::fs::read_dir(&workspace_root) {
        Ok(entries) => {
            let count = entries.filter_map(Result::ok).count();
            format!("Workspace scan completed. Found {count} top-level entries.")
        }
        Err(e) => format!("Workspace scan failed: {e}"),
    };
    set_step(&run_id, 1, "done", scan_detail);

    if is_stopped(&run_id) {
        return;
    }
    set_step(
        &run_id,
        2,
        "running",
        "Searching relevant code paths from goal keywords.".to_string(),
    );
    let goal = {
        let store = match runs_store().lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        match store.get(&run_id) {
            Some(r) => r.goal.clone(),
            None => return,
        }
    };
    let keyword = goal.split_whitespace().find(|w| w.len() >= 3).unwrap_or("agent");
    let search_detail = match Command::new("rg")
        .args(["-n", "--no-heading", "--color", "never", keyword, "src"])
        .current_dir(&workspace_root)
        .output()
    {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            let files: std::collections::BTreeSet<String> = text
                .lines()
                .filter_map(|l| l.split(':').next())
                .map(|s| s.to_string())
                .take(10)
                .collect();
            if files.is_empty() {
                format!("No direct matches for '{keyword}'.")
            } else {
                format!("Found {} candidate files for '{keyword}': {}", files.len(), files.into_iter().collect::<Vec<_>>().join(", "))
            }
        }
        Err(e) => {
            if e.kind() == ErrorKind::NotFound {
                "ripgrep (rg) was not found. Install it and add it to your PATH, or use Discover Files in the Agent panel (same requirement).".to_string()
            } else {
                format!("Could not run ripgrep (rg): {e}")
            }
        }
    };
    set_step(&run_id, 2, "done", search_detail);

    if is_stopped(&run_id) {
        return;
    }
    set_step(
        &run_id,
        3,
        "running",
        "Running git status --short.".to_string(),
    );
    let verify_detail = match Command::new("git")
        .args(["status", "--short"])
        .current_dir(&workspace_root)
        .output()
    {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if text.is_empty() {
                "Working tree clean (git status --short).".to_string()
            } else {
                format!(
                    "{} changed or untracked paths (git status --short).",
                    text.lines().count()
                )
            }
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            format!(
                "git status failed: {}",
                if err.is_empty() { "unknown error" } else { &err }
            )
        }
        Err(e) => format!("Could not run git: {e}"),
    };
    set_step(&run_id, 3, "done", verify_detail);

    finish_run(
        &run_id,
        "done",
        "Run complete. Use Discover Files to search, then Generate draft and Apply to edit.".to_string(),
    );
}

fn sanitize_workspace_path(workspace_root: &Path, input: &str) -> Result<PathBuf, String> {
    let root = fs::canonicalize(workspace_root)
        .map_err(|e| format!("Workspace not accessible: {e}"))?;
    let raw = Path::new(input.trim());
    if raw.as_os_str().is_empty() {
        return Err("Path is empty.".to_string());
    }

    let resolved = if raw.is_absolute() {
        let mut out = PathBuf::new();
        for c in raw.components() {
            match c {
                Component::Prefix(_) | Component::RootDir => out.push(c),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !out.pop() {
                        return Err("Path escapes workspace.".to_string());
                    }
                }
                Component::Normal(x) => out.push(x),
            }
        }
        out
    } else {
        let mut out = root.clone();
        for c in raw.components() {
            match c {
                Component::CurDir => {}
                Component::ParentDir => {
                    if !out.pop() || !out.starts_with(&root) {
                        return Err("Path escapes workspace.".to_string());
                    }
                }
                Component::Normal(x) => out.push(x),
                Component::Prefix(_) | Component::RootDir => {
                    return Err("Invalid relative path.".to_string());
                }
            }
        }
        out
    };

    if !resolved.starts_with(&root) {
        return Err("Path is outside workspace.".to_string());
    }
    Ok(resolved)
}
