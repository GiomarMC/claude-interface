//! Bridge between the UI and the `claude` CLI running in bidirectional
//! stream-json mode. Each line Claude Code prints is forwarded to the
//! webview as a `claude-event`; the UI writes protocol messages back
//! through `send_line`.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::thread;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

pub struct Session {
    child: Child,
    stdin: ChildStdin,
}

pub struct Sessions(pub Mutex<HashMap<String, Session>>);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartOptions {
    key: String,
    cwd: String,
    model: Option<String>,
    permission_mode: Option<String>,
    effort: Option<String>,
    resume: Option<String>,
}

#[derive(Clone, Serialize)]
struct EventPayload {
    key: String,
    line: String,
}

/// Desktop launchers often start apps with a minimal PATH, so look in the
/// usual install locations as well.
fn find_claude() -> PathBuf {
    let exe = if cfg!(windows) { "claude.exe" } else { "claude" };
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(exe);
            if candidate.is_file() {
                return candidate;
            }
            #[cfg(windows)]
            {
                let cmd = dir.join("claude.cmd");
                if cmd.is_file() {
                    return cmd;
                }
            }
        }
    }
    if let Some(home) = home_dir() {
        for rel in [
            ".local/bin",
            ".claude/local",
            ".npm-global/bin",
            ".bun/bin",
            "AppData/Roaming/npm",
        ] {
            let candidate = home.join(rel).join(exe);
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from(exe)
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[tauri::command]
pub fn start_session(
    app: AppHandle,
    state: State<Sessions>,
    opts: StartOptions,
) -> Result<(), String> {
    stop(&state, &opts.key);

    let mut cmd = Command::new(find_claude());
    cmd.args([
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--permission-prompt-tool",
        "stdio",
    ]);
    if let Some(m) = opts.model.filter(|s| !s.is_empty()) {
        cmd.args(["--model", &m]);
    }
    if let Some(m) = opts.permission_mode.filter(|s| !s.is_empty()) {
        cmd.args(["--permission-mode", &m]);
    }
    if let Some(e) = opts.effort.filter(|s| !s.is_empty()) {
        cmd.args(["--effort", &e]);
    }
    if let Some(r) = opts.resume.filter(|s| !s.is_empty()) {
        cmd.args(["--resume", &r]);
    }
    cmd.current_dir(&opts.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("No se pudo iniciar `claude`: {e}"))?;
    let stdin = child.stdin.take().ok_or("sin stdin")?;
    let stdout = child.stdout.take().ok_or("sin stdout")?;
    let stderr = child.stderr.take().ok_or("sin stderr")?;

    let key = opts.key.clone();
    let out_app = app.clone();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            let _ = out_app.emit(
                "claude-event",
                EventPayload { key: key.clone(), line },
            );
        }
        let _ = out_app.emit("claude-exit", key);
    });

    let key = opts.key.clone();
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            let _ = app.emit(
                "claude-stderr",
                EventPayload { key: key.clone(), line },
            );
        }
    });

    state
        .0
        .lock()
        .unwrap()
        .insert(opts.key, Session { child, stdin });
    Ok(())
}

#[tauri::command]
pub fn send_line(state: State<Sessions>, key: String, line: String) -> Result<(), String> {
    let mut sessions = state.0.lock().unwrap();
    let session = sessions.get_mut(&key).ok_or("La sesión no está activa")?;
    session
        .stdin
        .write_all(line.as_bytes())
        .and_then(|_| session.stdin.write_all(b"\n"))
        .and_then(|_| session.stdin.flush())
        .map_err(|e| format!("Error escribiendo a claude: {e}"))
}

fn stop(state: &State<Sessions>, key: &str) {
    if let Some(mut s) = state.0.lock().unwrap().remove(key) {
        let _ = s.child.kill();
        let _ = s.child.wait();
    }
}

#[tauri::command]
pub fn stop_session(state: State<Sessions>, key: String) {
    stop(&state, &key);
}
