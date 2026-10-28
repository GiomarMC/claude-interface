//! Lee la configuración del emulador de terminal del usuario (fuente,
//! fondo, texto y paleta ANSI de 16 colores) para que la app se vea como
//! Claude Code dentro de esa terminal.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::claude::home_dir;

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct TerminalStyle {
    /// Identificador del emulador: "konsole", "kitty", …
    terminal: String,
    /// Perfil o tema del que salen los colores.
    profile: Option<String>,
    font_family: Option<String>,
    /// En puntos tipográficos, como los configuran las terminales.
    font_size: Option<f32>,
    background: Option<String>,
    foreground: Option<String>,
    /// 16 colores: 0–7 normales, 8–15 brillantes. Vacío si no se encontró.
    palette: Vec<String>,
}

pub const TERMINALS: &[&str] = &[
    "konsole",
    "gnome-terminal",
    "kitty",
    "alacritty",
    "ghostty",
    "foot",
    "windows-terminal",
];

fn hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Acepta "#rrggbb", "rrggbb", "0xrrggbb", "#rgb", "rgb(r,g,b)" y "r,g,b".
fn parse_color(s: &str) -> Option<String> {
    let s = s.trim().trim_matches(|c| c == '\'' || c == '"');
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|x| x.strip_suffix(')')) {
        return parse_color(inner);
    }
    if s.contains(',') {
        let p: Vec<u8> = s.split(',').take(3).filter_map(|x| x.trim().parse().ok()).collect();
        return (p.len() == 3).then(|| hex(p[0], p[1], p[2]));
    }
    let h = s.trim_start_matches('#').trim_start_matches("0x");
    match h.len() {
        6 => u32::from_str_radix(h, 16).ok().map(|_| format!("#{}", h.to_lowercase())),
        3 => {
            let mut out = String::from("#");
            for c in h.chars() {
                out.push(c);
                out.push(c);
            }
            u32::from_str_radix(h, 16).ok().map(|_| out.to_lowercase())
        }
        // GNOME/X11: #rrrrggggbbbb
        12 => {
            let p: Vec<u8> = (0..3)
                .filter_map(|i| u8::from_str_radix(&h[i * 4..i * 4 + 2], 16).ok())
                .collect();
            (p.len() == 3).then(|| hex(p[0], p[1], p[2]))
        }
        _ => None,
    }
}

/// INI sencillo: [sección] clave=valor.
fn parse_ini(text: &str) -> Vec<(String, String, String)> {
    let mut section = String::new();
    let mut out = vec![];
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with(';') || line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
        } else if let Some((k, v)) = line.split_once('=') {
            out.push((section.clone(), k.trim().to_string(), v.trim().to_string()));
        }
    }
    out
}

fn ini_get<'a>(ini: &'a [(String, String, String)], section: &str, key: &str) -> Option<&'a str> {
    ini.iter()
        .find(|(s, k, _)| s == section && k == key)
        .map(|(_, _, v)| v.as_str())
}

fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join(".config")))
}

fn data_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![];
    if let Some(d) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        dirs.push(d);
    } else if let Some(h) = home_dir() {
        dirs.push(h.join(".local/share"));
    }
    let sys = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dirs.extend(sys.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    dirs
}

// ── Konsole ─────────────────────────────────────────────────────────

/// Esquema Breeze, que viene compilado dentro de Konsole.
const BREEZE: [&str; 18] = [
    "#232627", "#fcfcfc", // fondo, texto
    "#232627", "#ed1515", "#11d116", "#f67400", "#1d99f3", "#9b59b6", "#1abc9c", "#fcfcfc",
    "#7f8c8d", "#c0392b", "#1cdc9a", "#fdbc4b", "#3daee9", "#8e44ad", "#16a085", "#ffffff",
];

fn breeze(style: &mut TerminalStyle) {
    style.background = Some(BREEZE[0].into());
    style.foreground = Some(BREEZE[1].into());
    style.palette = BREEZE[2..].iter().map(|s| s.to_string()).collect();
}

fn konsole() -> Option<TerminalStyle> {
    let rc = fs::read_to_string(config_home()?.join("konsolerc")).ok();
    let profile_name = rc
        .as_deref()
        .map(parse_ini)
        .and_then(|ini| ini_get(&ini, "Desktop Entry", "DefaultProfile").map(String::from));
    let dirs: Vec<PathBuf> = data_dirs().into_iter().map(|d| d.join("konsole")).collect();
    let installed = rc.is_some() || dirs.iter().any(|d| d.is_dir());
    if !installed {
        return None;
    }

    let mut style = TerminalStyle { terminal: "konsole".into(), ..Default::default() };
    let profile = profile_name
        .as_ref()
        .and_then(|name| dirs.iter().find_map(|d| fs::read_to_string(d.join(name)).ok()))
        .map(|t| parse_ini(&t))
        .unwrap_or_default();

    // Font=Familia,tamaño,...
    let font = ini_get(&profile, "Appearance", "Font").unwrap_or("Hack,10");
    let mut parts = font.split(',');
    style.font_family = parts.next().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    style.font_size = parts.next().and_then(|s| s.trim().parse().ok());

    let scheme = ini_get(&profile, "Appearance", "ColorScheme").unwrap_or("Breeze").to_string();
    style.profile = Some(match &profile_name {
        Some(p) => format!("{} · {scheme}", p.trim_end_matches(".profile")),
        None => scheme.clone(),
    });
    let scheme_file = dirs
        .iter()
        .find_map(|d| fs::read_to_string(d.join(format!("{scheme}.colorscheme"))).ok());
    let Some(text) = scheme_file else {
        breeze(&mut style);
        return Some(style);
    };
    let ini = parse_ini(&text);
    let color = |section: &str| ini_get(&ini, section, "Color").and_then(parse_color);
    style.background = color("Background");
    style.foreground = color("Foreground");
    let mut palette = vec![];
    for i in 0..16 {
        // Formato estándar: Color0..7 y Color0Intense..7Intense; algunos usan Color8..15.
        let c = if i < 8 {
            color(&format!("Color{i}"))
        } else {
            color(&format!("Color{}Intense", i - 8)).or_else(|| color(&format!("Color{i}")))
        };
        palette.push(c.unwrap_or_else(|| BREEZE[2 + i].into()));
    }
    style.palette = palette;
    Some(style)
}

// ── GNOME Terminal ──────────────────────────────────────────────────

fn gsettings(args: &[&str]) -> Option<String> {
    let out = Command::new("gsettings").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().trim_matches('\'').to_string())
}

fn gnome_terminal() -> Option<TerminalStyle> {
    let id = gsettings(&["get", "org.gnome.Terminal.ProfilesList", "default"])?;
    let schema = format!("org.gnome.Terminal.Legacy.Profile:/org/gnome/terminal/legacy/profiles:/:{id}/");
    let get = |key: &str| gsettings(&["get", &schema, key]);

    let mut style = TerminalStyle {
        terminal: "gnome-terminal".into(),
        profile: get("visible-name"),
        ..Default::default()
    };
    let font = if get("use-system-font").as_deref() == Some("false") {
        get("font")
    } else {
        gsettings(&["get", "org.gnome.desktop.interface", "monospace-font-name"])
    };
    if let Some(font) = font {
        // "Familia Estilo 11"
        let mut words: Vec<&str> = font.split_whitespace().collect();
        if let Some(size) = words.last().and_then(|s| s.parse::<f32>().ok()) {
            style.font_size = Some(size);
            words.pop();
        }
        style.font_family = Some(words.join(" "));
    }
    if get("use-theme-colors").as_deref() == Some("false") {
        style.background = get("background-color").and_then(|c| parse_color(&c));
        style.foreground = get("foreground-color").and_then(|c| parse_color(&c));
    }
    if let Some(p) = get("palette") {
        style.palette = p
            .trim_matches(|c| c == '[' || c == ']')
            .split(',')
            .filter_map(parse_color)
            .collect();
    }
    Some(style)
}

// ── kitty ───────────────────────────────────────────────────────────

fn kitty_read(path: &Path, style: &mut TerminalStyle, depth: u8) {
    let Ok(text) = fs::read_to_string(path) else { return };
    for line in text.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once(char::is_whitespace) else { continue };
        let value = value.trim();
        match key {
            "include" | "globinclude" if depth < 3 => {
                let inc = path.parent().unwrap_or(Path::new(".")).join(value);
                kitty_read(&inc, style, depth + 1);
            }
            "font_family" if value != "monospace" => style.font_family = Some(value.into()),
            "font_size" => style.font_size = value.parse().ok(),
            "background" => style.background = parse_color(value),
            "foreground" => style.foreground = parse_color(value),
            k if k.starts_with("color") => {
                if let (Ok(i), Some(c)) = (k[5..].parse::<usize>(), parse_color(value)) {
                    if i < 16 {
                        if style.palette.len() < 16 {
                            style.palette.resize(16, String::new());
                        }
                        style.palette[i] = c;
                    }
                }
            }
            _ => {}
        }
    }
}

fn kitty() -> Option<TerminalStyle> {
    let path = config_home()?.join("kitty/kitty.conf");
    path.is_file().then_some(())?;
    let mut style = TerminalStyle { terminal: "kitty".into(), ..Default::default() };
    kitty_read(&path, &mut style, 0);
    Some(style)
}

// ── Alacritty ───────────────────────────────────────────────────────

fn alacritty_read(path: &Path, style: &mut TerminalStyle, depth: u8) {
    let Some(doc) = fs::read_to_string(path).ok().and_then(|t| t.parse::<toml::Table>().ok()) else {
        return;
    };
    let imports = doc
        .get("general")
        .and_then(|g| g.get("import"))
        .or_else(|| doc.get("import"))
        .and_then(|i| i.as_array());
    if depth < 3 {
        for imp in imports.into_iter().flatten().filter_map(|v| v.as_str()) {
            let p = imp.replace('~', &home_dir().unwrap_or_default().to_string_lossy());
            alacritty_read(&path.parent().unwrap_or(Path::new(".")).join(p), style, depth + 1);
        }
    }
    let at = |keys: &[&str]| {
        let mut v = toml::Value::Table(doc.clone());
        for k in keys {
            v = v.get(*k)?.clone();
        }
        Some(v)
    };
    if let Some(f) = at(&["font", "normal", "family"]).and_then(|v| v.as_str().map(String::from)) {
        style.font_family = Some(f);
    }
    if let Some(size) = at(&["font", "size"]).and_then(|v| v.as_float().or(v.as_integer().map(|i| i as f64))) {
        style.font_size = Some(size as f32);
    }
    if let Some(c) = at(&["colors", "primary", "background"]).and_then(|v| v.as_str().and_then(parse_color)) {
        style.background = Some(c);
    }
    if let Some(c) = at(&["colors", "primary", "foreground"]).and_then(|v| v.as_str().and_then(parse_color)) {
        style.foreground = Some(c);
    }
    const NAMES: [&str; 8] = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
    for (offset, group) in [(0, "normal"), (8, "bright")] {
        for (i, name) in NAMES.iter().enumerate() {
            if let Some(c) = at(&["colors", group, name]).and_then(|v| v.as_str().and_then(parse_color)) {
                if style.palette.len() < 16 {
                    style.palette.resize(16, String::new());
                }
                style.palette[offset + i] = c;
            }
        }
    }
}

fn alacritty() -> Option<TerminalStyle> {
    let base = config_home()?.join("alacritty");
    let path = [base.join("alacritty.toml"), base.join("alacritty.yml")]
        .into_iter()
        .find(|p| p.is_file())
        .or_else(|| {
            let win = std::env::var_os("APPDATA").map(PathBuf::from)?.join("alacritty/alacritty.toml");
            win.is_file().then_some(win)
        })?;
    let mut style = TerminalStyle { terminal: "alacritty".into(), ..Default::default() };
    alacritty_read(&path, &mut style, 0);
    Some(style)
}

// ── Ghostty ─────────────────────────────────────────────────────────

fn ghostty() -> Option<TerminalStyle> {
    let dir = config_home()?.join("ghostty");
    let text = fs::read_to_string(dir.join("config")).ok()?;
    let mut style = TerminalStyle { terminal: "ghostty".into(), ..Default::default() };
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    // theme = nombre → ~/.config/ghostty/themes/nombre (los temas integrados no se pueden leer)
    if let Some(theme) = lines
        .iter()
        .find_map(|l| l.split_once('=').filter(|(k, _)| k.trim() == "theme").map(|(_, v)| v.trim().to_string()))
    {
        style.profile = Some(theme.clone());
        if let Ok(t) = fs::read_to_string(dir.join("themes").join(&theme)) {
            let mut themed: Vec<String> = t.lines().map(String::from).collect();
            themed.append(&mut lines);
            lines = themed;
        }
    }
    for line in &lines {
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "font-family" => style.font_family = Some(v.trim_matches('"').into()),
            "font-size" => style.font_size = v.parse().ok(),
            "background" => style.background = parse_color(v),
            "foreground" => style.foreground = parse_color(v),
            "palette" => {
                if let Some((i, c)) = v.split_once('=') {
                    if let (Ok(i), Some(c)) = (i.trim().parse::<usize>(), parse_color(c)) {
                        if i < 16 {
                            if style.palette.len() < 16 {
                                style.palette.resize(16, String::new());
                            }
                            style.palette[i] = c;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Some(style)
}

// ── foot ────────────────────────────────────────────────────────────

fn foot() -> Option<TerminalStyle> {
    let text = fs::read_to_string(config_home()?.join("foot/foot.ini")).ok()?;
    let ini = parse_ini(&text);
    let mut style = TerminalStyle { terminal: "foot".into(), ..Default::default() };
    if let Some(font) = ini_get(&ini, "", "font").or_else(|| ini_get(&ini, "main", "font")) {
        // "Familia:size=11,Fallback"
        let first = font.split(',').next().unwrap_or(font);
        let mut parts = first.split(':');
        style.font_family = parts.next().map(|s| s.trim().to_string());
        style.font_size = parts
            .find_map(|p| p.strip_prefix("size="))
            .and_then(|s| s.parse().ok());
    }
    style.background = ini_get(&ini, "colors", "background").and_then(parse_color);
    style.foreground = ini_get(&ini, "colors", "foreground").and_then(parse_color);
    let mut palette = vec![];
    for i in 0..16 {
        let key = if i < 8 { format!("regular{i}") } else { format!("bright{}", i - 8) };
        palette.push(ini_get(&ini, "colors", &key).and_then(parse_color).unwrap_or_default());
    }
    if palette.iter().any(|c| !c.is_empty()) {
        style.palette = palette;
    }
    Some(style)
}

// ── Windows Terminal ────────────────────────────────────────────────

const CAMPBELL: [&str; 18] = [
    "#0c0c0c", "#cccccc",
    "#0c0c0c", "#c50f1f", "#13a10e", "#c19c00", "#0037da", "#881798", "#3a96dd", "#cccccc",
    "#767676", "#e74856", "#16c60c", "#f9f1a5", "#3b78ff", "#b4009e", "#61d6d6", "#f2f2f2",
];

/// Quita comentarios // y /* */ de un JSONC sin tocar cadenas.
fn strip_jsonc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_str = false;
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
            out.push(c);
        } else if c == '/' && chars.peek() == Some(&'/') {
            while chars.next().is_some_and(|n| n != '\n') {}
            out.push('\n');
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut prev = ' ';
            for n in chars.by_ref() {
                if prev == '*' && n == '/' {
                    break;
                }
                prev = n;
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn windows_terminal() -> Option<TerminalStyle> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    let text = [
        "Packages/Microsoft.WindowsTerminal_8wekyb3d8bbwe/LocalState/settings.json",
        "Packages/Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe/LocalState/settings.json",
        "Microsoft/Windows Terminal/settings.json",
    ]
    .iter()
    .find_map(|p| fs::read_to_string(local.join(p)).ok())?;
    let v: serde_json::Value = serde_json::from_str(&strip_jsonc(&text)).ok()?;
    let default_id = v.get("defaultProfile").and_then(|d| d.as_str()).unwrap_or("");
    let profiles = v.get("profiles");
    let defaults = profiles.and_then(|p| p.get("defaults"));
    let list = profiles
        .and_then(|p| p.get("list").or(Some(p)))
        .and_then(|l| l.as_array())
        .cloned()
        .unwrap_or_default();
    let profile = list
        .iter()
        .find(|p| p.get("guid").and_then(|g| g.as_str()) == Some(default_id))
        .or_else(|| list.first());
    let field = |key: &str| -> Option<serde_json::Value> {
        profile.and_then(|p| p.get(key)).or_else(|| defaults.and_then(|d| d.get(key))).cloned()
    };

    let mut style = TerminalStyle {
        terminal: "windows-terminal".into(),
        profile: profile.and_then(|p| p.get("name")).and_then(|n| n.as_str()).map(String::from),
        font_family: Some("Cascadia Mono".into()),
        font_size: Some(12.0),
        ..Default::default()
    };
    if let Some(font) = field("font") {
        if let Some(face) = font.get("face").and_then(|f| f.as_str()) {
            style.font_family = Some(face.into());
        }
        if let Some(size) = font.get("size").and_then(|s| s.as_f64()) {
            style.font_size = Some(size as f32);
        }
    }
    let scheme_name = field("colorScheme")
        .and_then(|c| c.as_str().map(String::from).or_else(|| c.get("dark")?.as_str().map(String::from)))
        .unwrap_or_else(|| "Campbell".into());
    let scheme = v
        .get("schemes")
        .and_then(|s| s.as_array())
        .and_then(|s| s.iter().find(|x| x.get("name").and_then(|n| n.as_str()) == Some(&scheme_name)));
    style.background = Some(CAMPBELL[0].into());
    style.foreground = Some(CAMPBELL[1].into());
    style.palette = CAMPBELL[2..].iter().map(|s| s.to_string()).collect();
    if let Some(s) = scheme {
        const KEYS: [&str; 16] = [
            "black", "red", "green", "yellow", "blue", "purple", "cyan", "white",
            "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue",
            "brightPurple", "brightCyan", "brightWhite",
        ];
        let col = |k: &str| s.get(k).and_then(|c| c.as_str()).and_then(parse_color);
        if let Some(c) = col("background") {
            style.background = Some(c);
        }
        if let Some(c) = col("foreground") {
            style.foreground = Some(c);
        }
        for (i, k) in KEYS.iter().enumerate() {
            if let Some(c) = col(k) {
                style.palette[i] = c;
            }
        }
    }
    if let Some(bg) = field("background").and_then(|b| b.as_str().and_then(parse_color)) {
        style.background = Some(bg);
    }
    if let Some(fg) = field("foreground").and_then(|b| b.as_str().and_then(parse_color)) {
        style.foreground = Some(fg);
    }
    style.profile = Some(match style.profile {
        Some(p) => format!("{p} · {scheme_name}"),
        None => scheme_name,
    });
    Some(style)
}

// ── Selección ───────────────────────────────────────────────────────

fn read(name: &str) -> Option<TerminalStyle> {
    match name {
        "konsole" => konsole(),
        "gnome-terminal" => gnome_terminal(),
        "kitty" => kitty(),
        "alacritty" => alacritty(),
        "ghostty" => ghostty(),
        "foot" => foot(),
        "windows-terminal" => windows_terminal(),
        _ => None,
    }
}

/// Orden de búsqueda en modo automático: primero la terminal propia del
/// escritorio, después las configuradas a mano.
fn auto_order() -> Vec<&'static str> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();
    let mut order: Vec<&str> = vec![];
    if cfg!(windows) {
        order.push("windows-terminal");
    } else if desktop.contains("KDE") {
        order.push("konsole");
    } else if desktop.contains("GNOME") || desktop.contains("UNITY") || desktop.contains("CINNAMON") {
        order.push("gnome-terminal");
    }
    for t in ["kitty", "alacritty", "ghostty", "foot", "konsole", "gnome-terminal"] {
        if !order.contains(&t) {
            order.push(t);
        }
    }
    order
}

/// `which`: "auto" o el nombre de una terminal concreta.
#[tauri::command]
pub async fn terminal_style(which: String) -> Option<TerminalStyle> {
    let mut style = if which == "auto" {
        auto_order().into_iter().find_map(read)
    } else {
        read(&which)
    }?;
    if style.palette.len() != 16 || style.palette.iter().any(|c| c.is_empty()) {
        style.palette.clear();
    }
    Some(style)
}

/// Terminales con configuración encontrada en este equipo.
#[tauri::command]
pub async fn detected_terminals() -> Vec<String> {
    TERMINALS
        .iter()
        .filter(|t| read(t).is_some())
        .map(|t| t.to_string())
        .collect()
}
