//! Primer arranque: comprueba que Claude Code esté instalado y con sesión
//! iniciada, y si no, lo instala con el instalador oficial y lanza el login
//! sin que el usuario tenga que abrir una terminal.

use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::thread;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::claude::{claude_command, find_claude_installed};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeStatus {
    installed: bool,
    path: Option<String>,
    version: Option<String>,
    /// None si no se pudo saber (versión antigua, proveedor externo…):
    /// en ese caso la app no bloquea el arranque.
    logged_in: Option<bool>,
    auth_method: Option<String>,
}

#[tauri::command]
pub async fn claude_status() -> ClaudeStatus {
    tauri::async_runtime::spawn_blocking(|| {
        let Some(path) = find_claude_installed() else {
            return ClaudeStatus {
                installed: false,
                path: None,
                version: None,
                logged_in: None,
                auth_method: None,
            };
        };
        let version = claude_command(&path)
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().next().map(String::from));
        let auth: Option<Value> = claude_command(&path)
            .args(["auth", "status", "--json"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| serde_json::from_slice(&o.stdout).ok());
        let provider = auth.as_ref().and_then(|a| a.get("apiProvider")?.as_str().map(String::from));
        let logged_in = auth
            .as_ref()
            .and_then(|a| a.get("loggedIn")?.as_bool())
            // Bedrock, Vertex, etc. no usan login de Anthropic.
            .map(|v| v || provider.as_deref().is_some_and(|p| p != "firstParty"))
            // Con una API key en el entorno tampoco hace falta.
            .map(|v| v || std::env::var_os("ANTHROPIC_API_KEY").is_some());
        ClaudeStatus {
            installed: true,
            path: Some(path.to_string_lossy().into_owned()),
            version,
            logged_in,
            auth_method: auth.and_then(|a| a.get("authMethod")?.as_str().map(String::from)),
        }
    })
    .await
    .unwrap_or(ClaudeStatus { installed: false, path: None, version: None, logged_in: None, auth_method: None })
}

/// Proceso de instalación o login en curso.
pub struct SetupTask(pub Mutex<Option<(Child, Option<ChildStdin>)>>);

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for n in chars.by_ref() {
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out
}

/// Reenvía la salida en trozos (no por líneas: el login pide el código sin
/// salto de línea al final).
fn pipe_output(app: AppHandle, mut reader: impl Read + Send + 'static) {
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            let _ = app.emit("setup-output", strip_ansi(&String::from_utf8_lossy(&buf[..n])));
        }
    });
}

fn spawn_task(app: AppHandle, task: &State<SetupTask>, mut cmd: Command) -> Result<(), String> {
    let mut guard = task.0.lock().unwrap();
    if let Some((mut old, _)) = guard.take() {
        let _ = old.kill();
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    pipe_output(app.clone(), child.stdout.take().ok_or("sin stdout")?);
    pipe_output(app.clone(), child.stderr.take().ok_or("sin stderr")?);
    let stdin = child.stdin.take();
    *guard = Some((child, stdin));
    drop(guard);

    // Avisa a la UI cuando termina, con su código de salida.
    thread::spawn(move || loop {
        thread::sleep(std::time::Duration::from_millis(200));
        let task = app.state::<SetupTask>();
        let mut guard = task.0.lock().unwrap();
        let Some((child, _)) = guard.as_mut() else {
            let _ = app.emit("setup-exit", Option::<i32>::None);
            return;
        };
        if let Ok(Some(status)) = child.try_wait() {
            *guard = None;
            let _ = app.emit("setup-exit", status.code());
            return;
        }
    });
    Ok(())
}

/// Instalador oficial de Claude Code (no necesita permisos de administrador).
#[tauri::command]
pub fn install_claude(app: AppHandle, task: State<SetupTask>) -> Result<(), String> {
    let cmd = if cfg!(windows) {
        let mut c = Command::new("powershell");
        c.args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "irm https://claude.ai/install.ps1 | iex",
        ]);
        c
    } else {
        let mut c = Command::new("/bin/bash");
        c.args(["-c", "curl -fsSL https://claude.ai/install.sh | bash"]);
        c
    };
    spawn_task(app, &task, cmd)
}

/// `claude auth login`: abre el navegador e imprime la URL; si hace falta,
/// el código se pega con `setup_input`.
#[tauri::command]
pub fn login_claude(app: AppHandle, task: State<SetupTask>, console: bool) -> Result<(), String> {
    let path = find_claude_installed().ok_or("Claude Code no está instalado")?;
    let mut cmd = claude_command(&path);
    cmd.args(["auth", "login"]);
    if console {
        cmd.arg("--console");
    }
    spawn_task(app, &task, cmd)
}

#[tauri::command]
pub fn setup_input(task: State<SetupTask>, text: String) -> Result<(), String> {
    let mut guard = task.0.lock().unwrap();
    let stdin = guard
        .as_mut()
        .and_then(|(_, s)| s.as_mut())
        .ok_or("No hay ningún proceso esperando")?;
    stdin
        .write_all(format!("{}\n", text.trim()).as_bytes())
        .and_then(|_| stdin.flush())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn setup_cancel(task: State<SetupTask>) {
    if let Some((mut child, _)) = task.0.lock().unwrap().take() {
        let _ = child.kill();
    }
}
