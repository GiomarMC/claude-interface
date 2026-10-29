//! Terminal real en cada panel: un pseudo-terminal con el shell del usuario
//! (zsh, bash, fish, PowerShell…), dibujado con xterm.js en la página.
//!
//! El shell se abre con una pequeña integración que se carga después de la
//! configuración del usuario:
//! - una función `claude` que, en vez de abrir el TUI, avisa a la app con la
//!   secuencia OSC 633 "ClaudeInterface;<cwd>;<args>" (en base64) y espera
//!   respuesta: al instante "ok" (abre la interfaz) o "run" (abre el Claude
//!   Code de siempre), y "cd:<ruta>" al salir de la interfaz de la app. Si la
//!   confirmación no llega (p. ej. una consola que se come la secuencia), la
//!   función abre el TUI en vez de quedarse esperando (`claude-interface`
//!   hace lo mismo, para no abrir otra ventana desde dentro de la app);
//! - OSC 7 con el directorio actual, para abrir pestañas y paneles nuevos ahí.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use base64::Engine;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

pub struct Ptys(pub Mutex<HashMap<String, Pty>>);

#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PtyEvent {
    Data { data: String },
    Exit,
}

const ZSHENV: &str = r#"CI_ZDOTDIR="$ZDOTDIR"
ZDOTDIR="${CI_USER_ZDOTDIR:-$HOME}"
[[ -f "$ZDOTDIR/.zshenv" ]] && source "$ZDOTDIR/.zshenv"
CI_USER_ZDOTDIR="$ZDOTDIR"
ZDOTDIR="$CI_ZDOTDIR"
"#;

const ZPROFILE: &str = r#"ZDOTDIR="$CI_USER_ZDOTDIR"
[[ -f "$ZDOTDIR/.zprofile" ]] && source "$ZDOTDIR/.zprofile"
ZDOTDIR="$CI_ZDOTDIR"
"#;

// Después de .zshrc, ZDOTDIR vuelve a ser el del usuario (y zsh lee su .zlogin).
const ZSHRC: &str = r#"ZDOTDIR="$CI_USER_ZDOTDIR"
[[ -f "$ZDOTDIR/.zshrc" ]] && source "$ZDOTDIR/.zshrc"
source "$CI_ZDOTDIR/integration.sh"
unset CI_ZDOTDIR CI_USER_ZDOTDIR
autoload -Uz add-zsh-hook
add-zsh-hook precmd __ci_osc7
add-zsh-hook chpwd __ci_osc7
"#;

const BASHRC: &str = r#"if [[ "$OSTYPE" == darwin* && -f ~/.bash_profile ]]; then source ~/.bash_profile
elif [[ -f ~/.bashrc ]]; then source ~/.bashrc; fi
source "$(dirname "${BASH_SOURCE[0]}")/integration.sh"
PROMPT_COMMAND="__ci_osc7${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
"#;

/// Común a zsh y bash.
const INTEGRATION_SH: &str = r#"__ci_b64() { base64 | tr -d '\n'; }
__ci_osc7() { printf '\033]7;file://%s%s\007' "${HOSTNAME:-$HOST}" "$PWD"; }
unalias claude 2>/dev/null
claude() {
  if [[ ! -t 0 || ! -t 1 ]]; then command claude "$@"; return; fi
  printf '\033]633;ClaudeInterface;%s;%s\007' "$(printf '%s' "$PWD" | __ci_b64)" "$(printf '%s\0' "$@" | __ci_b64)"
  local __ci_r
  # La app confirma al instante ("ok" o "run"); si no llega, se abre el TUI.
  if ! IFS= read -rs -t 2 __ci_r || [[ "$__ci_r" != ok ]]; then command claude "$@"; return; fi
  IFS= read -rs __ci_r
  [[ "$__ci_r" == cd:* && "${__ci_r#cd:}" != "$PWD" ]] && builtin cd -- "${__ci_r#cd:}"
  return 0
}
# Dentro de la app, `claude-interface [carpeta]` abre la interfaz en este panel.
unalias claude-interface 2>/dev/null
claude-interface() {
  if [[ -n "$1" && -d "$1" ]]; then builtin cd -- "$1" || return; shift; fi
  claude "$@"
}
"#;

const FISH: &str = r#"functions -e claude 2>/dev/null
function claude
  if not isatty stdin; or not isatty stdout
    command claude $argv; return
  end
  printf '\e]633;ClaudeInterface;%s;%s\a' (printf '%s' $PWD | base64 | string join '') (printf '%s\0' $argv | base64 | string join '')
  read -s -l __ci_r
  if test "$__ci_r" != ok
    command claude $argv; return
  end
  read -s -l __ci_r
  if string match -q 'cd:*' -- $__ci_r
    set -l dir (string sub -s 4 -- $__ci_r)
    test "$dir" != "$PWD"; and cd $dir
  end
end
function claude-interface
  if test (count $argv) -gt 0; and test -d $argv[1]
    cd $argv[1]; or return
    set -e argv[1]
  end
  claude $argv
end
function __ci_osc7 --on-variable PWD
  printf '\e]7;file://%s%s\a' (hostname) $PWD
end
__ci_osc7
"#;

/// Sin eco: la respuesta de la app se lee tecla a tecla.
const PWSH: &str = r#"# UTF-8 en la consola (acentos, íconos) también en Windows PowerShell 5.1.
[Console]::InputEncoding = [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$OutputEncoding = [Console]::OutputEncoding

function __ci_ReadLine([int]$TimeoutMs = -1) {
  if ($TimeoutMs -ge 0) {
    $deadline = [DateTime]::Now.AddMilliseconds($TimeoutMs)
    while (-not [Console]::KeyAvailable) {
      if ([DateTime]::Now -gt $deadline) { return $null }
      Start-Sleep -Milliseconds 20
    }
  }
  $r = ''
  while ($true) {
    $k = [Console]::ReadKey($true)
    if ($k.Key -eq 'Enter') { return $r }
    $r += $k.KeyChar
  }
}

function claude {
  $real = (Get-Command claude -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1).Source
  if (-not $real) { Write-Host 'claude no está instalado: https://claude.com/claude-code'; return }
  if ([Console]::IsInputRedirected -or [Console]::IsOutputRedirected) { & $real @args; return }
  $utf8 = [Text.Encoding]::UTF8
  $p = [Convert]::ToBase64String($utf8.GetBytes($PWD.ProviderPath))
  $a = [Convert]::ToBase64String($utf8.GetBytes((($args | ForEach-Object { "$_" }) -join "`0")))
  [Console]::Write("$([char]27)]633;ClaudeInterface;$p;$a$([char]7)")
  # La app confirma al instante ("ok" o "run"); si no llega, se abre el TUI.
  if ((__ci_ReadLine 3000) -ne 'ok') { & $real @args; return }
  $r = __ci_ReadLine
  if ($r -like 'cd:*') { Set-Location -LiteralPath $r.Substring(3) }
}

function claude-interface {
  if ($args.Count -gt 0 -and (Test-Path -LiteralPath $args[0] -PathType Container)) {
    Set-Location -LiteralPath $args[0]
    $rest = @($args | Select-Object -Skip 1)
    claude @rest
  } else { claude @args }
}

# OSC 7 con la carpeta actual en cada prompt (también con oh-my-posh o starship).
$__ciPrompt = $function:prompt
function prompt {
  $loc = $executionContext.SessionState.Path.CurrentLocation
  if ($loc.Provider.Name -eq 'FileSystem') {
    [Console]::Write("$([char]27)]7;file://$env:COMPUTERNAME/$($loc.ProviderPath -replace '\\', '/')$([char]7)")
  }
  & $__ciPrompt
}
"#;

fn write_if_changed(path: &Path, content: &str) -> std::io::Result<()> {
    if fs::read_to_string(path).ok().as_deref() != Some(content) {
        fs::write(path, content)?;
    }
    Ok(())
}

/// Escribe los archivos de integración y devuelve su carpeta.
fn integration_dir(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_cache_dir().ok()?.join("shell");
    fs::create_dir_all(dir.join("zsh")).ok()?;
    let files = [
        ("integration.sh", INTEGRATION_SH),
        ("bashrc", BASHRC),
        ("integration.fish", FISH),
        ("zsh/.zshenv", ZSHENV),
        ("zsh/.zprofile", ZPROFILE),
        ("zsh/.zshrc", ZSHRC),
        ("zsh/integration.sh", INTEGRATION_SH),
    ];
    for (name, content) in files {
        write_if_changed(&dir.join(name), content).ok()?;
    }
    Some(dir)
}

fn which(exe: &str) -> Option<PathBuf> {
    std::env::split_paths(crate::shell_env::path())
        .map(|d| d.join(exe))
        .find(|p| p.is_file())
}

/// Shell del usuario con la integración cargada.
fn shell_command(app: &AppHandle) -> CommandBuilder {
    if cfg!(windows) {
        // PowerShell 7 si está (mejor soporte de colores y UTF-8); si no, el
        // Windows PowerShell que trae el sistema. La integración va en línea
        // (-EncodedCommand) y con la política de ejecución relajada solo para
        // este proceso: así funciona aunque los scripts estén bloqueados.
        let shell = which("pwsh.exe").unwrap_or_else(|| PathBuf::from("powershell.exe"));
        let mut cmd = CommandBuilder::new(shell);
        cmd.args(["-NoLogo", "-NoExit", "-ExecutionPolicy", "Bypass", "-EncodedCommand"]);
        let utf16: Vec<u8> = PWSH.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        cmd.arg(base64::engine::general_purpose::STANDARD.encode(utf16));
        return cmd;
    }
    let dir = integration_dir(app);
    let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        if cfg!(target_os = "macos") { "/bin/zsh".into() } else { "/bin/bash".into() }
    });
    let name = Path::new(&shell).file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    let mut cmd = CommandBuilder::new(&shell);
    // Las terminales de macOS abren shells de login; las de Linux, no.
    let login = cfg!(target_os = "macos");
    match (name.as_str(), dir) {
        ("zsh", Some(d)) => {
            let user = std::env::var("ZDOTDIR").ok().filter(|s| !s.is_empty());
            let home = crate::claude::home_dir().unwrap_or_default();
            cmd.env("CI_USER_ZDOTDIR", user.unwrap_or_else(|| home.to_string_lossy().into_owned()));
            cmd.env("ZDOTDIR", d.join("zsh"));
            if login {
                cmd.arg("-l");
            }
        }
        ("bash", Some(d)) => {
            cmd.arg("--rcfile");
            cmd.arg(d.join("bashrc"));
            cmd.arg("-i");
        }
        ("fish", Some(d)) => {
            if login {
                cmd.arg("-l");
            }
            cmd.arg("-C");
            cmd.arg(format!("source '{}'", d.join("integration.fish").display()));
        }
        _ => {
            if login {
                cmd.arg("-l");
            }
        }
    }
    cmd
}

#[tauri::command]
pub fn pty_spawn(
    app: AppHandle,
    state: State<Ptys>,
    key: String,
    cwd: String,
    cols: u16,
    rows: u16,
    on_event: Channel<PtyEvent>,
) -> Result<(), String> {
    kill(&state, &key);
    let size = PtySize { rows: rows.max(2), cols: cols.max(10), pixel_width: 0, pixel_height: 0 };
    let pair = native_pty_system().openpty(size).map_err(|e| e.to_string())?;
    let mut cmd = shell_command(&app);
    let dir = if Path::new(&cwd).is_dir() { PathBuf::from(&cwd) } else { crate::claude::home_dir().unwrap_or_default() };
    cmd.cwd(dir);
    cmd.env("PATH", crate::shell_env::path());
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("CLAUDE_INTERFACE", "1");
    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    std::thread::spawn(move || {
        let mut buf = [0u8; 16 * 1024];
        // Bytes de un carácter UTF-8 que quedó partido entre dos lecturas.
        let mut pending: Vec<u8> = vec![];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    pending.extend_from_slice(&buf[..n]);
                    let valid = match std::str::from_utf8(&pending) {
                        Ok(_) => pending.len(),
                        Err(e) if e.error_len().is_none() => e.valid_up_to(),
                        Err(_) => pending.len(),
                    };
                    let data = String::from_utf8_lossy(&pending[..valid]).into_owned();
                    pending.drain(..valid);
                    if !data.is_empty() && on_event.send(PtyEvent::Data { data }).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = on_event.send(PtyEvent::Exit);
    });

    state.0.lock().unwrap().insert(key, Pty { master: pair.master, writer, child });
    Ok(())
}

#[tauri::command]
pub fn pty_write(state: State<Ptys>, key: String, data: String) -> Result<(), String> {
    let mut ptys = state.0.lock().unwrap();
    let pty = ptys.get_mut(&key).ok_or("La terminal no está activa")?;
    pty.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    pty.writer.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pty_resize(state: State<Ptys>, key: String, cols: u16, rows: u16) {
    if let Some(pty) = state.0.lock().unwrap().get(&key) {
        let _ = pty.master.resize(PtySize { rows: rows.max(2), cols: cols.max(10), pixel_width: 0, pixel_height: 0 });
    }
}

fn kill(state: &Ptys, key: &str) {
    if let Some(mut p) = state.0.lock().unwrap().remove(key) {
        let _ = p.child.kill();
        let _ = p.child.wait();
    }
}

#[tauri::command]
pub fn pty_kill(state: State<Ptys>, key: String) {
    kill(&state, &key);
}

/// Cierra las terminales de una ventana que se cerró.
pub fn kill_window(state: &Ptys, label: &str) {
    let prefix = format!("{label}:");
    let keys: Vec<String> = state.0.lock().unwrap().keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
    for key in keys {
        kill(state, &key);
    }
}
