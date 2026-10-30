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

/// `pid` distingue las líneas de un proceso que se reemplazó (al reiniciar
/// la sesión) de las del nuevo, que comparte `key`.
#[derive(Clone, Serialize)]
struct EventPayload {
    key: String,
    pid: u32,
    line: String,
}

#[derive(Clone, Serialize)]
struct ExitPayload {
    key: String,
    pid: u32,
}

const CLAUDE_EXE: &str = if cfg!(windows) { "claude.exe" } else { "claude" };

/// Desktop launchers often start apps with a minimal PATH, so look in the
/// usual install locations as well.
fn find_claude() -> PathBuf {
    find_claude_installed().unwrap_or_else(|| PathBuf::from(CLAUDE_EXE))
}

/// Ruta de `claude` si está instalado (None si no se encuentra).
pub fn find_claude_installed() -> Option<PathBuf> {
    let exe = CLAUDE_EXE;
    for dir in std::env::split_paths(crate::shell_env::path()) {
        let candidate = dir.join(exe);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let cmd = dir.join("claude.cmd");
            if cmd.is_file() {
                return Some(cmd);
            }
        }
    }
    // Respaldo por si no se pudo leer el PATH del shell (ver shell_env.rs).
    let mut dirs: Vec<PathBuf> = ["/opt/homebrew/bin", "/usr/local/bin", "/snap/bin"]
        .into_iter()
        .map(PathBuf::from)
        .collect();
    if let Some(home) = home_dir() {
        for rel in [
            ".local/bin",
            ".claude/local",
            ".npm-global/bin",
            ".bun/bin",
            ".volta/bin",
            ".yarn/bin",
            ".local/share/pnpm",
            "Library/pnpm",
            "AppData/Roaming/npm",
        ] {
            dirs.push(home.join(rel));
        }
        // nvm: ~/.nvm/versions/node/<versión>/bin
        if let Ok(versions) = std::fs::read_dir(home.join(".nvm/versions/node")) {
            dirs.extend(versions.flatten().map(|v| v.path().join("bin")));
        }
    }
    dirs.into_iter().map(|d| d.join(exe)).find(|c| c.is_file())
}

/// `Command` para claude sin abrir una consola en Windows.
pub fn claude_command(path: &std::path::Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = crate::shell_env::command(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
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
) -> Result<u32, String> {
    stop(&state, &opts.key);

    let mut cmd = crate::shell_env::command(find_claude());
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
    let pid = child.id();

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
                EventPayload { key: key.clone(), pid, line },
            );
        }
        let _ = out_app.emit("claude-exit", ExitPayload { key, pid });
    });

    let key = opts.key.clone();
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            let _ = app.emit(
                "claude-stderr",
                EventPayload { key: key.clone(), pid, line },
            );
        }
    });

    state
        .0
        .lock()
        .unwrap()
        .insert(opts.key, Session { child, stdin });
    Ok(pid)
}

/// `claude --version` → "2.1.312"
#[tauri::command]
pub async fn claude_version() -> Option<String> {
    let out = claude_command(&find_claude()).arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace().next().map(String::from)
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

/// Termina las sesiones de una ventana que se cerró.
pub fn stop_window(state: &Sessions, label: &str) {
    let prefix = format!("{label}:");
    let mut sessions = state.0.lock().unwrap();
    let keys: Vec<String> = sessions.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
    for key in keys {
        if let Some(mut s) = sessions.remove(&key) {
            let _ = s.child.kill();
            let _ = s.child.wait();
        }
    }
}

#[tauri::command]
pub fn stop_session(state: State<Sessions>, key: String) {
    stop(&state, &key);
}
