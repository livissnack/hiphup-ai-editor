use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::Emitter;

#[derive(Clone)]
pub struct TerminalSessions(pub Arc<Mutex<HashMap<String, TerminalSession>>>);

pub struct TerminalSession {
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub child: Box<dyn portable_pty::Child + Send>,
}

fn default_shell() -> (String, Vec<String>) {
    ("powershell.exe".to_string(), vec!["-NoLogo".to_string()])
}

#[tauri::command]
pub fn terminal_create(
    app: tauri::AppHandle,
    state: tauri::State<TerminalSessions>,
    cwd: Option<String>,
) -> Result<serde_json::Value, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let title = "PowerShell".to_string();

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    let (exe, args) = default_shell();
    let mut cmd = CommandBuilder::new(exe);
    for a in args {
        cmd.arg(a);
    }
    if let Some(cwd) = cwd {
        if !cwd.trim().is_empty() {
            cmd.cwd(PathBuf::from(cwd));
        }
    }
    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    let app_for_thread = app.clone();
    let id_for_thread = id.clone();
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app_for_thread.emit(
                        "terminal-output",
                        serde_json::json!({ "id": id_for_thread, "data": data }),
                    );
                }
                Err(_) => break,
            }
        }
    });

    state.0.lock().map_err(|e| e.to_string())?.insert(
        id.clone(),
        TerminalSession {
            master: pair.master,
            writer,
            child,
        },
    );

    Ok(serde_json::json!({ "id": id, "title": title }))
}

#[tauri::command]
pub fn terminal_write(
    state: tauri::State<TerminalSessions>,
    id: String,
    data: String,
) -> Result<(), String> {
    let mut sessions = state.0.lock().map_err(|e| e.to_string())?;
    let session = sessions.get_mut(&id).ok_or("terminal not found")?;
    session.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    session.writer.flush().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn terminal_resize(
    state: tauri::State<TerminalSessions>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let mut sessions = state.0.lock().map_err(|e| e.to_string())?;
    let session = sessions.get_mut(&id).ok_or("terminal not found")?;
    session
        .master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn terminal_kill(state: tauri::State<TerminalSessions>, id: String) -> Result<(), String> {
    let mut sessions = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(mut sess) = sessions.remove(&id) {
        let _ = sess.child.kill();
    }
    Ok(())
}

