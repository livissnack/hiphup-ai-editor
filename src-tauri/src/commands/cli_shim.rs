use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliOpenEntry {
    pub path: String,
    pub is_directory: bool,
}

pub struct CliLaunchState(pub Mutex<Option<Vec<CliOpenEntry>>>);

fn resolve_cli_path_arg(arg: &str) -> Option<PathBuf> {
    let p = Path::new(arg);
    let combined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(p)
    };
    if !combined.exists() {
        return None;
    }
    std::fs::canonicalize(combined).ok()
}

/// Collect file/folder paths from process args (after the executable). Skips flags and URLs.
pub fn parse_cli_launch_entries() -> Option<Vec<CliOpenEntry>> {
    let mut out = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg.starts_with('-') {
            continue;
        }
        if arg.starts_with("http://") || arg.starts_with("https://") {
            continue;
        }
        let resolved = resolve_cli_path_arg(&arg)?;
        let md = std::fs::metadata(&resolved).ok()?;
        out.push(CliOpenEntry {
            path: resolved.to_string_lossy().to_string(),
            is_directory: md.is_dir(),
        });
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[tauri::command]
pub fn take_cli_launch_paths(state: State<CliLaunchState>) -> Option<Vec<CliOpenEntry>> {
    state.0.lock().ok()?.take()
}

#[cfg(windows)]
fn write_windows_shim(exe: &Path, cli_dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(cli_dir).map_err(|e| e.to_string())?;
    let cmd_path = cli_dir.join("ai-editor.cmd");
    let exe_str = exe.to_string_lossy();
    let body = format!("@echo off\r\n\"{exe_str}\" %*\r\n");
    std::fs::write(&cmd_path, body).map_err(|e| e.to_string())?;
    Ok(cmd_path)
}

#[cfg(windows)]
fn append_user_path_entry(dir: &Path) -> Result<(), String> {
    let dir_str = dir.to_str().ok_or("CLI directory path is not valid UTF-8")?;
    let escaped = dir_str.replace('\'', "''");
    let ps = format!(
        "$d='{}'; $u=[Environment]::GetEnvironmentVariable('Path','User'); if ($null -eq $u) {{ $u = '' }}; if ($u -notlike ('*'+$d+'*')) {{ $n = if ($u) {{ $u + ';' + $d }} else {{ $d }}; [Environment]::SetEnvironmentVariable('Path',$n,'User') }}",
        escaped
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
        .output()
        .map_err(|e| format!("Failed to run PowerShell: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}

#[cfg(not(windows))]
fn write_unix_shim(exe: &Path, cli_dir: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(cli_dir).map_err(|e| e.to_string())?;
    let script_path = cli_dir.join("ai-editor");
    let exe_str = exe.to_string_lossy();
    let body = format!("#!/bin/sh\nexec \"{exe_str}\" \"$@\"\n");
    std::fs::write(&script_path, body).map_err(|e| e.to_string())?;
    let mut perm = std::fs::metadata(&script_path)
        .map_err(|e| e.to_string())?
        .permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(&script_path, perm).map_err(|e| e.to_string())?;
    Ok(script_path)
}

/// Writes a small launcher script and, on Windows, appends its directory to the user PATH.
#[tauri::command]
pub fn install_cli_in_path(app: AppHandle) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?;
    let cli_dir = base.join("cli");

    #[cfg(windows)]
    {
        let shim = write_windows_shim(&exe, &cli_dir)?;
        append_user_path_entry(&cli_dir)?;
        return Ok(format!(
            "Installed launcher at {}. Open a new terminal and run: ai-editor . or ai-editor <file>\n\
             (PATH was updated for your user account.)",
            shim.display()
        ));
    }
    #[cfg(not(windows))]
    {
        let shim = write_unix_shim(&exe, &cli_dir)?;
        Ok(format!(
            "Installed launcher at {}.\n\
             Add this directory to your PATH, for example:\n\
             export PATH=\"$PATH:{}\"\n\
             Then run: ai-editor . or ai-editor <file>",
            shim.display(),
            cli_dir.display()
        ))
    }
}
