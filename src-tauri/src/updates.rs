//! Actualizaciones de instalaciones .rpm/.deb en Linux. El plugin de Tauri
//! solo sabe reemplazar AppImage (y los instaladores de Windows/macOS), así
//! que para los paquetes del sistema descargamos el paquete nuevo del release
//! y lo instalamos con pkexec, que pide la contraseña en una ventana.

use std::path::PathBuf;
use std::process::Command;

const REPO: &str = "GiomarMC/claude-interface";

fn succeeds(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .output()
        .is_ok_and(|o| o.status.success())
}

fn has(cmd: &str) -> bool {
    succeeds("sh", &["-c", &format!("command -v {cmd}")])
}

/// Cómo se instaló la app: "appimage", "rpm", "deb", "bundle" (Windows,
/// macOS) o "dev" (build de desarrollo, sin actualizaciones).
#[tauri::command]
pub fn install_kind() -> String {
    if cfg!(debug_assertions) {
        return "dev".into();
    }
    if !cfg!(target_os = "linux") {
        return "bundle".into();
    }
    if std::env::var_os("APPIMAGE").is_some() {
        return "appimage".into();
    }
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    if has("rpm") && succeeds("rpm", &["-qf", &exe]) {
        "rpm".into()
    } else if has("dpkg") && succeeds("dpkg", &["-S", &exe]) {
        "deb".into()
    } else {
        "other".into()
    }
}

fn run(cmd: &mut Command) -> Result<(), String> {
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(match out.status.code() {
            // pkexec: el usuario canceló el diálogo de autenticación
            Some(126) | Some(127) => "Instalación cancelada".into(),
            _ => format!("Falló la instalación: {}", err.trim()),
        })
    }
}

/// Descarga el .rpm/.deb de la versión indicada y lo instala con pkexec.
#[tauri::command]
pub async fn install_linux_package(version: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if std::env::consts::ARCH != "x86_64" {
            return Err("Solo hay paquetes para x86_64; descárgalo a mano desde GitHub.".into());
        }
        let kind = install_kind();
        let asset = match kind.as_str() {
            "rpm" => format!("Claude.Interface-{version}-1.x86_64.rpm"),
            "deb" => format!("Claude.Interface_{version}_amd64.deb"),
            _ => return Err(format!("Instalación no soportada: {kind}")),
        };
        let url = format!("https://github.com/{REPO}/releases/download/v{version}/{asset}");
        let file: PathBuf = std::env::temp_dir().join(&asset);
        run(Command::new("curl").args(["-fsSL", "-o"]).arg(&file).arg(&url))
            .map_err(|e| format!("No se pudo descargar {url}: {e}"))?;

        let mut cmd = Command::new("pkexec");
        if kind == "rpm" {
            if has("dnf") {
                cmd.args(["dnf", "install", "-y"]);
            } else {
                cmd.args(["rpm", "-U"]);
            }
        } else if has("apt-get") {
            cmd.args(["apt-get", "install", "-y"]);
        } else {
            cmd.args(["dpkg", "-i"]);
        }
        let result = run(cmd.arg(&file));
        let _ = std::fs::remove_file(&file);
        result
    })
    .await
    .map_err(|e| e.to_string())?
}
