//! Local helpers the UI needs without spending Claude's tokens: reading
//! attachments, git status for the status line and session history.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

use base64::Engine;
use serde::Serialize;
use serde_json::Value;

use crate::claude::home_dir;

/// Larger text files are passed by path so Claude reads only what it needs.
const MAX_INLINE_TEXT: u64 = 256 * 1024;
/// The API rejects images above ~5 MB.
const MAX_IMAGE: u64 = 5 * 1024 * 1024;
const MAX_PDF: u64 = 20 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    name: String,
    path: String,
    /// "text" | "image" | "pdf" | "dir" | "ref"
    kind: String,
    mime: String,
    size: u64,
    data: String,
    note: Option<String>,
}

fn image_mime(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

#[tauri::command]
pub fn read_attachment(path: String) -> Result<Attachment, String> {
    let p = PathBuf::from(&path);
    let meta = fs::metadata(&p).map_err(|e| format!("{path}: {e}"))?;
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    let size = meta.len();
    let mut att = Attachment {
        name,
        path: path.clone(),
        kind: "ref".into(),
        mime: String::new(),
        size,
        data: String::new(),
        note: None,
    };

    if meta.is_dir() {
        att.kind = "dir".into();
        return Ok(att);
    }

    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let b64 = base64::engine::general_purpose::STANDARD;

    if let Some(mime) = image_mime(&ext) {
        if size > MAX_IMAGE {
            att.note = Some("imagen > 5 MB, se pasa la ruta".into());
            return Ok(att);
        }
        att.kind = "image".into();
        att.mime = mime.into();
        att.data = b64.encode(fs::read(&p).map_err(|e| e.to_string())?);
        return Ok(att);
    }

    if ext == "pdf" {
        if size > MAX_PDF {
            att.note = Some("PDF > 20 MB, se pasa la ruta".into());
            return Ok(att);
        }
        att.kind = "pdf".into();
        att.mime = "application/pdf".into();
        att.data = b64.encode(fs::read(&p).map_err(|e| e.to_string())?);
        return Ok(att);
    }

    if size > MAX_INLINE_TEXT {
        att.note = Some("archivo grande, se pasa la ruta".into());
        return Ok(att);
    }
    let bytes = fs::read(&p).map_err(|e| e.to_string())?;
    match String::from_utf8(bytes) {
        Ok(text) if !text.contains('\0') => {
            att.kind = "text".into();
            att.mime = "text/plain".into();
            att.data = text;
        }
        _ => att.note = Some("binario, se pasa la ruta".into()),
    }
    Ok(att)
}

#[derive(Serialize, Default)]
pub struct GitInfo {
    repo: String,
    branch: Option<String>,
    insertions: u32,
    deletions: u32,
}

fn git(cwd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .arg("--no-optional-locks")
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn number_before(text: &str, word: &str) -> u32 {
    text.split(',')
        .find(|part| part.contains(word))
        .and_then(|part| part.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

#[tauri::command]
pub fn git_info(cwd: String) -> GitInfo {
    let root = git(&cwd, &["rev-parse", "--show-toplevel"]).unwrap_or_else(|| cwd.clone());
    let repo = Path::new(&root)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or(root);
    let branch = git(&cwd, &["symbolic-ref", "--short", "HEAD"]);
    let stat = branch
        .as_ref()
        .and_then(|_| git(&cwd, &["diff", "--shortstat", "HEAD"]))
        .unwrap_or_default();
    GitInfo {
        repo,
        branch,
        insertions: number_before(&stat, "insertion"),
        deletions: number_before(&stat, "deletion"),
    }
}

/// Claude Code stores transcripts in ~/.claude/projects/<cwd with every
/// non-alphanumeric character replaced by '-'>.
fn project_dir(cwd: &str) -> Option<PathBuf> {
    let encoded: String = cwd
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    Some(home_dir()?.join(".claude").join("projects").join(encoded))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    id: String,
    title: String,
    modified: u64,
    messages: u32,
}

fn first_user_text(v: &Value) -> Option<String> {
    let content = v.get("message")?.get("content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .find_map(|i| (i.get("type")? == "text").then(|| i.get("text")?.as_str().map(String::from))?)?,
        _ => return None,
    };
    let text = text.trim();
    (!text.is_empty() && !text.starts_with('<')).then(|| text.chars().take(90).collect())
}

#[tauri::command]
pub fn list_sessions(cwd: String) -> Vec<SessionSummary> {
    let Some(dir) = project_dir(&cwd) else { return vec![] };
    let Ok(entries) = fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<SessionSummary> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|e| {
            let path = e.path();
            let modified = e
                .metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs();
            let file = fs::File::open(&path).ok()?;
            let mut title = None;
            let mut first = None;
            let mut messages = 0;
            for line in BufReader::new(file).lines().map_while(Result::ok) {
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                match v.get("type").and_then(Value::as_str) {
                    Some("ai-title") => {
                        title = v.get("aiTitle").and_then(Value::as_str).map(String::from)
                    }
                    Some("custom-title") => {
                        title = v.get("customTitle").and_then(Value::as_str).map(String::from)
                    }
                    Some("user") => {
                        if first.is_none() {
                            first = first_user_text(&v);
                        }
                        messages += 1;
                    }
                    _ => {}
                }
            }
            let title = title.or(first)?;
            Some(SessionSummary {
                id: path.file_stem()?.to_string_lossy().into_owned(),
                title,
                modified,
                messages,
            })
        })
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out.truncate(50);
    out
}

/// Returns the user/assistant messages of a transcript so the UI can render
/// the history of a resumed session.
#[tauri::command]
pub fn load_session(cwd: String, id: String) -> Result<Vec<Value>, String> {
    let dir = project_dir(&cwd).ok_or("sin HOME")?;
    let file = fs::File::open(dir.join(format!("{id}.jsonl"))).map_err(|e| e.to_string())?;
    Ok(BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str::<Value>(&l).ok())
        .filter(|v| {
            matches!(v.get("type").and_then(Value::as_str), Some("user" | "assistant"))
                && !v.get("isSidechain").and_then(Value::as_bool).unwrap_or(false)
                && !v.get("isMeta").and_then(Value::as_bool).unwrap_or(false)
        })
        .collect())
}

#[derive(Serialize)]
pub struct ShellOutput {
    output: String,
    code: Option<i32>,
}

/// Modo `!`: ejecuta el comando localmente, sin pasar por Claude.
#[tauri::command]
pub async fn run_shell(cwd: String, command: String) -> Result<ShellOutput, String> {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", &command]);
        c
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let mut c = Command::new(shell);
        c.args(["-c", &command]);
        c
    };
    let out = cmd.current_dir(cwd).output().map_err(|e| e.to_string())?;
    let mut output = String::from_utf8_lossy(&out.stdout).into_owned();
    output.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok(ShellOutput { output, code: out.status.code() })
}

#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    fs::write(path, content).map_err(|e| e.to_string())
}

/// Directory passed on the command line (`claude-interface ~/proyecto`), if any.
#[tauri::command]
pub fn initial_cwd() -> Option<String> {
    let arg = std::env::args().skip(1).find(|a| !a.starts_with('-'))?;
    let path = fs::canonicalize(arg).ok()?;
    path.is_dir().then(|| path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn user_settings() -> Value {
    home_dir()
        .and_then(|h| fs::read_to_string(h.join(".claude").join("settings.json")).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}
