//! Las apps abiertas desde el Dock/Finder (macOS) o algunos lanzadores de
//! Linux no heredan el PATH de la terminal: solo /usr/bin:/bin:… Así no se
//! encuentra un `claude` instalado con Homebrew, npm/nvm, etc., ni Claude Code
//! ve las herramientas del usuario. Calculamos el PATH del shell de login del
//! usuario (en segundo plano, al arrancar) y se lo pasamos a cada proceso.

use std::process::Command;
use std::sync::OnceLock;

static PATH: OnceLock<String> = OnceLock::new();

/// Empieza a calcular el PATH sin bloquear el arranque.
pub fn warm_up() {
    std::thread::spawn(|| {
        path();
    });
}

/// PATH completo del usuario (espera si aún se está calculando).
pub fn path() -> &'static str {
    PATH.get_or_init(resolve)
}

/// `Command` con el PATH del usuario.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    cmd.env("PATH", path());
    cmd
}

fn has_claude(path: &str) -> bool {
    let exe = if cfg!(windows) { "claude.exe" } else { "claude" };
    std::env::split_paths(path).any(|d| d.join(exe).is_file())
}

#[cfg(unix)]
fn resolve() -> String {
    let current = std::env::var("PATH").unwrap_or_default();
    // En Linux el lanzador suele pasar el PATH de la sesión: si ya se ve
    // claude no hace falta lanzar ningún shell. En macOS (Dock) nunca basta.
    if !cfg!(target_os = "macos") && has_claude(&current) {
        return current;
    }
    let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        if cfg!(target_os = "macos") { "/bin/zsh".into() } else { "/bin/bash".into() }
    });
    // Primero sin modo interactivo (rápido; basta para Homebrew, que va en
    // .zprofile); si así no aparece claude, con -i para leer .zshrc/.bashrc,
    // donde suelen configurarse nvm y compañía.
    let found = shell_path(&shell, "-lc")
        .filter(|p| has_claude(p))
        .or_else(|| shell_path(&shell, "-ilc"));
    match found {
        Some(p) => merge(&p, &current),
        None => current,
    }
}

#[cfg(not(unix))]
fn resolve() -> String {
    std::env::var("PATH").unwrap_or_default()
}

/// PATH del shell + el que ya teníamos, sin duplicados.
#[cfg(unix)]
fn merge(first: &str, second: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    first
        .split(':')
        .chain(second.split(':'))
        .filter(|p| !p.is_empty() && seen.insert(*p))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(unix)]
fn shell_path(shell: &str, flags: &str) -> Option<String> {
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    const MARK: &str = "__CLAUDE_INTERFACE_ENV__";
    // `env` funciona igual en bash, zsh y fish; las marcas separan lo que
    // impriman los archivos de configuración del shell.
    let script = format!("echo {MARK}; /usr/bin/env; echo {MARK}");
    let mut child = Command::new(shell)
        .args([flags, &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    // Un .zshrc lento o que se queda esperando no debe bloquear la app.
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < Duration::from_secs(5) => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let out = child.wait_with_output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let env = text.split(MARK).nth(1)?;
    env.lines().find_map(|l| l.strip_prefix("PATH=")).map(String::from)
}
