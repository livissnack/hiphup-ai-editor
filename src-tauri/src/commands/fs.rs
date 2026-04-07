use encoding_rs::Encoding;
use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

fn to_tree_node(path: &Path) -> Result<TreeNode, String> {
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    let is_dir = metadata.is_dir();
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_string();
    let path_str = path.to_string_lossy().to_string();

    Ok(TreeNode {
        name,
        path: path_str,
        is_dir,
    })
}

#[tauri::command]
pub fn list_dir(path: String) -> Result<Vec<TreeNode>, String> {
    let base = Path::new(&path);
    if !base.is_dir() {
        return Err("Path is not a directory".to_string());
    }

    let mut nodes = Vec::new();
    let entries = fs::read_dir(base).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        nodes.push(to_tree_node(&entry.path())?);
    }

    nodes.sort_by(|a, b| {
        if a.is_dir == b.is_dir {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        } else if a.is_dir {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });

    Ok(nodes)
}

#[tauri::command]
pub fn search_files_by_name(path: String, keyword: String, limit: Option<usize>) -> Result<Vec<TreeNode>, String> {
    let base = Path::new(&path);
    if !base.is_dir() {
        return Err("Path is not a directory".to_string());
    }
    let needle = keyword.trim().to_lowercase();
    if needle.is_empty() {
        return Ok(Vec::new());
    }
    let max_count = limit.unwrap_or(200).clamp(1, 2000);
    let mut out: Vec<TreeNode> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![base.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for entry in entries {
            let entry = match entry {
                Ok(v) => v,
                Err(_) => continue,
            };
            let p = entry.path();
            let meta = match entry.metadata() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let is_dir = meta.is_dir();
            if is_dir {
                stack.push(p);
                continue;
            }
            let name = entry
                .file_name()
                .to_string_lossy()
                .to_string();
            let path_text = entry.path().to_string_lossy().to_string();
            let hay_name = name.to_lowercase();
            let hay_path = path_text.to_lowercase();
            if hay_name.contains(&needle) || hay_path.contains(&needle) {
                out.push(TreeNode {
                    name,
                    path: path_text,
                    is_dir: false,
                });
                if out.len() >= max_count {
                    return Ok(out);
                }
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn read_file(path: String) -> Result<String, String> {
    read_file_with_encoding(path, "UTF-8".to_string())
}

#[tauri::command]
pub fn write_file(path: String, content: String) -> Result<(), String> {
    write_file_with_encoding(path, content, "UTF-8".to_string())
}

fn resolve_encoding(label: &str) -> Result<&'static Encoding, String> {
    let normalized = label
        .trim()
        .to_ascii_lowercase()
        .replace('_', "-")
        .replace(' ', "");

    let canonical = match normalized.as_str() {
        "utf8" | "utf-8" => "utf-8",
        "utf16le" | "utf-16le" => "utf-16le",
        "utf16be" | "utf-16be" => "utf-16be",
        "gbk" | "gb2312" => "gbk",
        "big5" => "big5",
        "shiftjis" | "shift-jis" | "sjis" => "shift_jis",
        "eucjp" | "euc-jp" => "euc-jp",
        "euckr" | "euc-kr" => "euc-kr",
        "windows1252" | "windows-1252" | "cp1252" => "windows-1252",
        "iso88591" | "iso-8859-1" | "latin1" => "windows-1252",
        "koi8r" | "koi8-r" => "koi8-r",
        _ => normalized.as_str(),
    };

    Encoding::for_label(canonical.as_bytes())
        .ok_or_else(|| format!("Unsupported encoding: {label}"))
}

#[tauri::command]
pub fn read_file_with_encoding(path: String, encoding: String) -> Result<String, String> {
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let enc = resolve_encoding(&encoding)?;
    let (text, _, had_errors) = enc.decode(&bytes);
    if had_errors {
        return Err(format!(
            "Decoding error: file cannot be fully decoded as {encoding}"
        ));
    }
    Ok(text.into_owned())
}

#[tauri::command]
pub fn write_file_with_encoding(path: String, content: String, encoding: String) -> Result<(), String> {
    let enc = resolve_encoding(&encoding)?;
    let (bytes, _, had_errors) = enc.encode(&content);
    if had_errors {
        return Err(format!(
            "Encoding error: content contains characters not representable in {encoding}"
        ));
    }
    fs::write(path, bytes.as_ref()).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDataUrl {
    pub mime: String,
    pub data_url: String,
}

fn mime_from_path(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") {
        return "image/png";
    }
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        return "image/jpeg";
    }
    if lower.ends_with(".gif") {
        return "image/gif";
    }
    if lower.ends_with(".webp") {
        return "image/webp";
    }
    if lower.ends_with(".bmp") {
        return "image/bmp";
    }
    if lower.ends_with(".ico") {
        return "image/x-icon";
    }
    "application/octet-stream"
}

#[tauri::command]
pub fn read_image_data_url(path: String) -> Result<ImageDataUrl, String> {
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let mime = mime_from_path(&path).to_string();
    let encoded = general_purpose::STANDARD.encode(bytes);
    let data_url = format!("data:{mime};base64,{encoded}");
    Ok(ImageDataUrl { mime, data_url })
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    let entries = fs::read_dir(src).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        if metadata.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn create_file(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if target.exists() {
        return Err("Target already exists".to_string());
    }
    fs::write(target, b"").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_folder(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if target.exists() {
        return Err("Target already exists".to_string());
    }
    fs::create_dir_all(target).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_path(old_path: String, new_path: String) -> Result<(), String> {
    let old = Path::new(&old_path);
    let newp = Path::new(&new_path);
    if !old.exists() {
        return Err("Source path does not exist".to_string());
    }
    if newp.exists() {
        return Err("Target path already exists".to_string());
    }
    if let Some(parent) = newp.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::rename(old, newp).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_path(path: String) -> Result<(), String> {
    let target = Path::new(&path);
    if !target.exists() {
        return Ok(());
    }
    let meta = fs::metadata(target).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        fs::remove_dir_all(target).map_err(|e| e.to_string())
    } else {
        fs::remove_file(target).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn copy_path(src: String, dst: String) -> Result<(), String> {
    let srcp = Path::new(&src);
    let dstp = Path::new(&dst);
    if !srcp.exists() {
        return Err("Source path does not exist".to_string());
    }
    if dstp.exists() {
        return Err("Target path already exists".to_string());
    }
    if let Some(parent) = dstp.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let meta = fs::metadata(srcp).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        copy_dir_recursive(srcp, dstp)
    } else {
        fs::copy(srcp, dstp).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[tauri::command]
pub fn move_path(src: String, dst: String) -> Result<(), String> {
    let srcp = PathBuf::from(src);
    let dstp = PathBuf::from(dst);
    if !srcp.exists() {
        return Err("Source path does not exist".to_string());
    }
    if dstp.exists() {
        return Err("Target path already exists".to_string());
    }
    if let Some(parent) = dstp.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::rename(srcp, dstp).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn path_exists(path: String) -> Result<bool, String> {
    Ok(Path::new(&path).exists())
}

#[tauri::command]
pub fn open_path_in_file_manager(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Open path is not supported on this platform".to_string())
}

#[tauri::command]
pub fn reveal_in_folder(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .args(["/select,", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Fallback: open containing folder on Linux.
        let parent = Path::new(&path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or(&path)
            .to_string();
        Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err("Reveal in folder is not supported on this platform".to_string())
}

