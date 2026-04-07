use serde::Serialize;
use std::collections::{HashMap};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[derive(Serialize, Clone)]
pub struct GitHead {
    pub branch: String,
    pub sha: String,
}

#[derive(Serialize, Clone)]
pub struct GitRefItem {
    pub name: String,
    pub sha: String,
    pub authored_unix: i64,
    pub author: String,
    pub message: String,
}

#[derive(Serialize, Clone)]
pub struct GitLogItem {
    pub sha: String,
    pub authored_unix: i64,
    pub rel_date: String,
    pub author: String,
    pub subject: String,
    pub decorations: String,
}

#[derive(Serialize, Clone)]
pub struct GitOverview {
    pub is_repo: bool,
    pub head: Option<GitHead>,
    pub locals: Vec<GitRefItem>,
    pub remotes: Vec<GitRefItem>,
    pub tags: Vec<GitRefItem>,
    pub log: Vec<GitLogItem>,
}

type CacheMap = HashMap<String, (Instant, GitOverview)>;
static GIT_OVERVIEW_CACHE: OnceLock<Mutex<CacheMap>> = OnceLock::new();
static GIT_TREE_DECORATIONS_CACHE: OnceLock<Mutex<HashMap<String, (Instant, HashMap<String, String>)>>> =
    OnceLock::new();

fn cache_key(path: &str, limit: u32) -> String {
    format!("{path}::{limit}")
}

fn run_git(cwd: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);

    // Avoid flashing a console window on Windows when running git frequently
    // (e.g. tree decorations / overview refresh) in the bundled app.
    #[cfg(target_os = "windows")]
    {
        // CREATE_NO_WINDOW
        cmd.creation_flags(0x08000000);
    }

    let out = cmd
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("git {:?} failed", args)
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[tauri::command]
pub fn git_exec(path: String, args: Vec<String>) -> Result<String, String> {
    let argv: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_git(&path, &argv)
}

fn parse_ref_items(text: &str) -> Vec<GitRefItem> {
    // Format: <name>\t<sha>\t<subject>
    text.lines()
        .filter_map(|line| {
            let line = line.trim_end();
            if line.is_empty() {
                return None;
            }
            let mut parts = line.split('\t');
            let name = parts.next()?.to_string();
            let sha = parts.next().unwrap_or("").to_string();
            let message = parts.next().unwrap_or("").to_string();
            Some(GitRefItem {
                name,
                sha,
                authored_unix: 0,
                author: String::new(),
                message,
            })
        })
        .collect()
}

fn enrich_ref_items(cwd: &str, items: &mut [GitRefItem]) {
    for item in items.iter_mut() {
        let out = run_git(cwd, &["log", "-1", "--format=%at\t%an", &item.name]).unwrap_or_default();
        let mut parts = out.trim_end().split('\t');
        let ts = parts
            .next()
            .unwrap_or("0")
            .parse::<i64>()
            .unwrap_or(0);
        let author = parts.next().unwrap_or("").to_string();
        item.authored_unix = ts;
        item.author = author;
    }
}

fn parse_log_items(text: &str) -> Vec<GitLogItem> {
    // Format: <sha>\t<unixTs>\t<relDate>\t<author>\t<subject>\t<decorations>
    text.lines()
        .filter_map(|line| {
            let line = line.trim_end();
            if line.is_empty() {
                return None;
            }
            let mut parts = line.split('\t');
            Some(GitLogItem {
                sha: parts.next().unwrap_or("").to_string(),
                authored_unix: parts
                    .next()
                    .unwrap_or("0")
                    .parse::<i64>()
                    .unwrap_or(0),
                rel_date: parts.next().unwrap_or("").to_string(),
                author: parts.next().unwrap_or("").to_string(),
                subject: parts.next().unwrap_or("").to_string(),
                decorations: parts.next().unwrap_or("").to_string(),
            })
        })
        .collect()
}

#[tauri::command]
pub fn git_overview(path: String, limit: Option<u32>) -> Result<GitOverview, String> {
    let limit = limit.unwrap_or(80).min(500);
    let key = cache_key(&path, limit);
    let cache = GIT_OVERVIEW_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some((saved_at, data)) = guard.get(&key) {
            if saved_at.elapsed() <= Duration::from_millis(1800) {
                return Ok(data.clone());
            }
        }
    }

    let is_repo = run_git(&path, &["rev-parse", "--is-inside-work-tree"])
        .map(|s| s.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !is_repo {
        let data = GitOverview {
            is_repo: false,
            head: None,
            locals: vec![],
            remotes: vec![],
            tags: vec![],
            log: vec![],
        };
        if let Ok(mut guard) = cache.lock() {
            guard.insert(key, (Instant::now(), data.clone()));
        }
        return Ok(data);
    }

    let branch = run_git(&path, &["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_string();
    let sha = run_git(&path, &["rev-parse", "HEAD"])?.trim().to_string();

    let locals_txt = run_git(
        &path,
        &[
            "for-each-ref",
            "refs/heads",
            "--format=%(refname:short)\t%(objectname:short)\t%(subject)",
        ],
    )
    .unwrap_or_default();
    let remotes_txt = run_git(
        &path,
        &[
            "for-each-ref",
            "refs/remotes",
            "--format=%(refname:short)\t%(objectname:short)\t%(subject)",
        ],
    )
    .unwrap_or_default();
    let tags_txt = run_git(
        &path,
        &[
            "for-each-ref",
            "refs/tags",
            "--format=%(refname:short)\t%(objectname:short)\t%(subject)",
        ],
    )
    .unwrap_or_default();

    let log_txt = run_git(
        &path,
        &[
            "log",
            &format!("-n{limit}"),
            "--date=relative",
            "--pretty=format:%h\t%at\t%ad\t%an\t%s\t%d",
            "--decorate=short",
        ],
    )
    .unwrap_or_default();

    let mut locals = parse_ref_items(&locals_txt);
    let mut remotes = parse_ref_items(&remotes_txt);
    let mut tags = parse_ref_items(&tags_txt);
    enrich_ref_items(&path, &mut locals);
    enrich_ref_items(&path, &mut remotes);
    enrich_ref_items(&path, &mut tags);

    let data = GitOverview {
        is_repo: true,
        head: Some(GitHead { branch, sha }),
        locals,
        remotes,
        tags,
        log: parse_log_items(&log_txt),
    };
    if let Ok(mut guard) = cache.lock() {
        guard.insert(key, (Instant::now(), data.clone()));
    }
    Ok(data)
}

#[tauri::command]
pub fn git_fetch(path: String) -> Result<String, String> {
    run_git(&path, &["fetch", "--all", "--prune"])
}

#[tauri::command]
pub fn git_pull(path: String) -> Result<String, String> {
    run_git(&path, &["pull", "--ff-only"])
}

#[tauri::command]
pub fn git_push(path: String) -> Result<String, String> {
    run_git(&path, &["push"])
}

#[tauri::command]
pub fn git_commit_all(path: String, message: String) -> Result<String, String> {
    if message.trim().is_empty() {
        return Err("Commit message is empty".to_string());
    }
    // Stage all (tracked + untracked), then commit.
    run_git(&path, &["add", "-A"])?;
    run_git(&path, &["commit", "-m", message.trim()])
}

#[tauri::command]
pub fn git_stash_save(path: String, message: Option<String>) -> Result<String, String> {
    let msg = message.unwrap_or_else(|| "WIP".to_string());
    // Include untracked files for a WebStorm-ish experience.
    run_git(&path, &["stash", "push", "-u", "-m", msg.trim()])
}

#[tauri::command]
pub fn git_stash_list(path: String) -> Result<Vec<String>, String> {
    let out = run_git(&path, &["stash", "list"])?;
    Ok(out
        .lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

#[tauri::command]
pub fn git_stash_pop(path: String) -> Result<String, String> {
    run_git(&path, &["stash", "pop"])
}

/// Map absolute file paths → decoration kind for the project tree (WebStorm/Cursor-style).
/// Keys use the same separators as `list_dir` (`Path` display on the current OS).
#[tauri::command]
pub fn git_tree_decorations(path: String) -> Result<HashMap<String, String>, String> {
    let repo = path.trim();
    if repo.is_empty() {
        return Ok(HashMap::new());
    }

    // This endpoint can get called frequently (tree loads, file ops, git ops).
    // Add a short TTL cache to avoid spawning `git` processes repeatedly.
    let ttl = Duration::from_millis(1500);
    let cache = GIT_TREE_DECORATIONS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock() {
        if let Some((saved_at, data)) = guard.get(repo) {
            if saved_at.elapsed() <= ttl {
                return Ok(data.clone());
            }
        }
    }
    fn abs(repo: &str, rel: &str) -> Option<String> {
        let t = rel.trim().trim_start_matches("./");
        if t.is_empty() {
            return None;
        }
        let mut p = PathBuf::from(repo);
        for part in t.split(['/', '\\']) {
            if part.is_empty() || part == "." {
                continue;
            }
            p.push(part);
        }
        Some(p.to_string_lossy().to_string())
    }

    // Single git process per refresh: parse porcelain v1 status output.
    //
    // - `--porcelain=v1` gives stable, machine-readable XY status pairs.
    // - `-z` uses NUL delimiters so paths are unambiguous.
    // - `--ignored=matching` includes ignored entries ("!!").
    // If `repo` is not a Git worktree, this command fails and we return an empty map.
    // Keep it as a single command per refresh.
    let status_z = run_git(repo, &["status", "--porcelain=v1", "-z", "--ignored=matching"])
        .unwrap_or_default();

    let bytes = status_z.as_bytes();
    let mut parts: Vec<&[u8]> = Vec::new();
    let mut start = 0usize;
    for (idx, b) in bytes.iter().enumerate() {
        if *b == 0 {
            parts.push(&bytes[start..idx]);
            start = idx + 1;
        }
    }

    let mut out: HashMap<String, String> = HashMap::new();

    let mut i = 0usize;
    while i < parts.len() {
        let p = parts[i];
        i += 1;
        if p.is_empty() {
            continue;
        }
        if p.len() < 4 {
            continue;
        }

        // Format: "XY <path>" (or for renames/copies: "R? <old>" NUL "<new>")
        let x = p[0] as char;
        let y = p[1] as char;
        if p[2] != b' ' {
            continue;
        }

        let mut path_bytes = &p[3..];

        // For renames/copies with -z, the old path is followed by a second NUL-terminated new path.
        if x == 'R' || x == 'C' {
            if i < parts.len() {
                let new_path = parts[i];
                i += 1;
                if !new_path.is_empty() {
                    path_bytes = new_path;
                }
            }
        }

        let rel = String::from_utf8_lossy(path_bytes).to_string();
        let Some(k) = abs(repo, &rel) else { continue };

        let label = if x == '!' && y == '!' {
            "ignored"
        } else if x == '?' && y == '?' {
            "untracked"
        } else if x == 'U'
            || y == 'U'
            || (x == 'A' && y == 'A')
            || (x == 'D' && y == 'D')
        {
            "conflict"
        } else {
            let staged = x != ' ';
            let worktree = y != ' ';
            if staged && worktree {
                "mixed"
            } else if staged {
                "staged"
            } else if worktree {
                "modified"
            } else {
                continue;
            }
        };

        out.insert(k, label.into());
    }

    if let Ok(mut guard) = cache.lock() {
        guard.insert(repo.to_string(), (Instant::now(), out.clone()));
    }
    Ok(out)
}

