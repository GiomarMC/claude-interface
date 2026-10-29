//! Atajos de teclado del emulador de terminal del usuario (pestañas,
//! divisiones, ventanas, zoom…) para que la app se maneje igual que esa
//! terminal. Se parte de los atajos por defecto de cada terminal, se
//! aplican los que el usuario haya cambiado en su configuración y lo que
//! falte se completa con los habituales del sistema operativo.
//!
//! Los atajos se normalizan como "ctrl+alt+shift+meta+tecla" (en ese orden,
//! en minúsculas): "ctrl+shift+t", "shift+right", "ctrl+(", "alt+1".

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::claude::home_dir;
use crate::terminal::{
    alacritty_conf, auto_order, config_home, ghostty_conf, gsettings, kitty_conf, read, strip_jsonc,
};

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Keymap {
    /// Terminal de la que salen los atajos (None: solo los del sistema).
    terminal: Option<String>,
    /// Acción → atajos.
    bindings: BTreeMap<String, Vec<String>>,
}

type Binds = HashMap<String, Vec<String>>;

// ── Normalización ───────────────────────────────────────────────────

fn modifier(tok: &str) -> Option<&'static str> {
    match tok {
        "ctrl" | "control" | "ctl" | "primary" => Some("ctrl"),
        "alt" | "opt" | "option" | "mod1" => Some("alt"),
        "shift" => Some("shift"),
        "meta" | "super" | "cmd" | "command" | "win" | "logo" | "hyper" => Some("meta"),
        _ => None,
    }
}

fn key_name(tok: &str) -> String {
    let t = tok.to_lowercase();
    match t.as_str() {
        "pgdown" | "pgdn" | "page_down" | "next" => "pagedown",
        "pgup" | "page_up" | "prior" => "pageup",
        "return" | "kp_enter" => "enter",
        "esc" => "escape",
        "ins" => "insert",
        "del" => "delete",
        "plus" | "kp_add" => "+",
        "minus" | "kp_subtract" => "-",
        "equal" | "equals" => "=",
        "parenleft" | "paren_left" => "(",
        "parenright" | "paren_right" => ")",
        "bracketleft" | "bracket_left" | "left_bracket" => "[",
        "bracketright" | "bracket_right" | "right_bracket" => "]",
        "braceleft" => "{",
        "braceright" => "}",
        "backslash" => "\\",
        "comma" => ",",
        "period" | "dot" => ".",
        "slash" => "/",
        "arrow_left" | "arrowleft" => "left",
        "arrow_right" | "arrowright" => "right",
        "arrow_up" | "arrowup" => "up",
        "arrow_down" | "arrowdown" => "down",
        _ => return t,
    }
    .to_string()
}

/// Une modificadores y tecla en el formato normalizado.
fn accel(mods: &[&str], key: &str) -> Option<String> {
    let key = key_name(key.trim());
    if key.is_empty() {
        return None;
    }
    let mut out = vec![];
    for m in ["ctrl", "alt", "shift", "meta"] {
        if mods.contains(&m) {
            out.push(m.to_string());
        }
    }
    // Qt llama "Backtab" a shift+tab.
    if key == "backtab" {
        if !out.iter().any(|m| m == "shift") {
            out.push("shift".into());
        }
        out.push("tab".into());
    } else {
        out.push(key);
    }
    Some(out.join("+"))
}

/// "Ctrl+Shift+T", "ctrl+shift+t", "Ctrl++" (separador `+`).
fn parse_plus(s: &str, kitty_mod: &[&'static str]) -> Option<String> {
    let s = s.trim();
    if s.is_empty() || s.contains('>') {
        return None;
    }
    let (rest, key) = if let Some(r) = s.strip_suffix("++") {
        (r, "+")
    } else {
        match s.rsplit_once('+') {
            Some((r, k)) => (r, k),
            None => ("", s),
        }
    };
    let mut mods: Vec<&str> = vec![];
    for tok in rest.split('+').filter(|t| !t.is_empty()) {
        let tok = tok.trim().to_lowercase();
        if tok == "kitty_mod" {
            mods.extend(kitty_mod.iter().copied());
        } else {
            mods.push(modifier(&tok)?);
        }
    }
    accel(&mods, key)
}

/// GTK: "<Primary><Shift>t", "<Control>Page_Down".
fn parse_gtk(s: &str) -> Option<String> {
    let s = s.trim().trim_matches('\'');
    if s.is_empty() || s == "disabled" {
        return None;
    }
    let mut mods = vec![];
    let mut rest = s;
    while let Some(r) = rest.strip_prefix('<') {
        let (m, after) = r.split_once('>')?;
        mods.push(modifier(&m.to_lowercase())?);
        rest = after;
    }
    accel(&mods, rest)
}

fn add(b: &mut Binds, action: &str, keys: &[&str]) {
    b.entry(action.into())
        .or_default()
        .extend(keys.iter().filter_map(|k| parse_plus(k, &["ctrl", "shift"])));
}

fn digits(b: &mut Binds, prefix: &str, count: usize) {
    for i in 1..=count {
        add(b, &format!("goto_tab_{i}"), &[&format!("{prefix}{i}")]);
    }
}

// ── Sistema operativo ───────────────────────────────────────────────

fn os_defaults() -> Binds {
    let mut b = Binds::new();
    if cfg!(target_os = "macos") {
        add(&mut b, "new_tab", &["meta+t"]);
        add(&mut b, "close_pane", &["meta+w"]);
        add(&mut b, "new_window", &["meta+n"]);
        add(&mut b, "next_tab", &["meta+shift+]", "ctrl+tab"]);
        add(&mut b, "prev_tab", &["meta+shift+[", "ctrl+shift+tab"]);
        digits(&mut b, "meta+", 9);
        add(&mut b, "split_right", &["meta+d"]);
        add(&mut b, "split_down", &["meta+shift+d"]);
        add(&mut b, "focus_left", &["meta+alt+left"]);
        add(&mut b, "focus_right", &["meta+alt+right"]);
        add(&mut b, "focus_up", &["meta+alt+up"]);
        add(&mut b, "focus_down", &["meta+alt+down"]);
        add(&mut b, "next_pane", &["meta+]"]);
        add(&mut b, "prev_pane", &["meta+["]);
        add(&mut b, "maximize_pane", &["meta+shift+enter"]);
        add(&mut b, "zoom_in", &["meta+=", "meta++"]);
        add(&mut b, "zoom_out", &["meta+-"]);
        add(&mut b, "zoom_reset", &["meta+0"]);
        add(&mut b, "export", &["meta+s"]);
        add(&mut b, "paste", &["meta+v"]);
    } else {
        add(&mut b, "new_tab", &["ctrl+shift+t"]);
        add(&mut b, "close_pane", &["ctrl+shift+w"]);
        add(&mut b, "new_window", &["ctrl+shift+n"]);
        add(&mut b, "next_tab", &["ctrl+tab", "ctrl+pagedown"]);
        add(&mut b, "prev_tab", &["ctrl+shift+tab", "ctrl+pageup"]);
        add(&mut b, "move_tab_left", &["ctrl+shift+pageup"]);
        add(&mut b, "move_tab_right", &["ctrl+shift+pagedown"]);
        digits(&mut b, "alt+", 9);
        // Los de Windows Terminal, que no chocan con la edición de texto.
        add(&mut b, "split_right", &["alt+shift+=", "alt+shift++"]);
        add(&mut b, "split_down", &["alt+shift+-"]);
        add(&mut b, "focus_left", &["alt+left"]);
        add(&mut b, "focus_right", &["alt+right"]);
        add(&mut b, "focus_up", &["alt+up"]);
        add(&mut b, "focus_down", &["alt+down"]);
        add(&mut b, "maximize_pane", &["ctrl+shift+enter"]);
        add(&mut b, "zoom_in", &["ctrl+=", "ctrl++"]);
        add(&mut b, "zoom_out", &["ctrl+-"]);
        add(&mut b, "zoom_reset", &["ctrl+0"]);
        add(&mut b, "export", &["ctrl+shift+s"]);
        add(&mut b, "copy", &["ctrl+shift+c"]);
        add(&mut b, "paste", &["ctrl+shift+v"]);
    }
    b
}

// ── Konsole ─────────────────────────────────────────────────────────

const KONSOLE_ACTIONS: &[(&str, &str)] = &[
    ("new-tab", "new_tab"),
    ("new-window", "new_window"),
    ("close-session", "close_pane"),
    ("close-active-view", "close_pane"),
    ("next-tab", "next_tab"),
    ("previous-tab", "prev_tab"),
    ("move-tab-to-left", "move_tab_left"),
    ("move-tab-to-right", "move_tab_right"),
    ("split-view-left-right", "split_right"),
    ("split-view-top-bottom", "split_down"),
    ("focus-view-left", "focus_left"),
    ("focus-view-right", "focus_right"),
    ("focus-view-above", "focus_up"),
    ("focus-view-below", "focus_down"),
    ("next-view", "next_pane"),
    ("previous-view", "prev_pane"),
    ("toggle-maximize-current-view", "maximize_pane"),
    ("enlarge-font", "zoom_in"),
    ("shrink-font", "zoom_out"),
    ("reset-font-size", "zoom_reset"),
    ("file_save_as", "export"),
    ("edit_copy", "copy"),
    ("edit_paste", "paste"),
];

fn konsole() -> Binds {
    let mut b = Binds::new();
    add(&mut b, "new_tab", &["Ctrl+Shift+T"]);
    add(&mut b, "new_window", &["Ctrl+Shift+N"]);
    add(&mut b, "close_pane", &["Ctrl+Shift+W", "Ctrl+Shift+X"]);
    // Ctrl+Tab es "última pestaña usada" en Konsole; con dos pestañas es lo mismo.
    add(&mut b, "next_tab", &["Shift+Right", "Ctrl+PgDown", "Ctrl+Tab"]);
    add(&mut b, "prev_tab", &["Shift+Left", "Ctrl+PgUp", "Ctrl+Shift+Tab"]);
    add(&mut b, "move_tab_left", &["Ctrl+Alt+Left"]);
    add(&mut b, "move_tab_right", &["Ctrl+Alt+Right"]);
    digits(&mut b, "Alt+", 9);
    add(&mut b, "split_right", &["Ctrl+("]);
    add(&mut b, "split_down", &["Ctrl+)"]);
    add(&mut b, "focus_left", &["Ctrl+Shift+Left"]);
    add(&mut b, "focus_right", &["Ctrl+Shift+Right"]);
    add(&mut b, "focus_up", &["Ctrl+Shift+Up"]);
    add(&mut b, "focus_down", &["Ctrl+Shift+Down"]);
    add(&mut b, "maximize_pane", &["Ctrl+Shift+E"]);
    add(&mut b, "zoom_in", &["Ctrl++", "Ctrl+="]);
    add(&mut b, "zoom_out", &["Ctrl+-"]);
    add(&mut b, "zoom_reset", &["Ctrl+0"]);
    add(&mut b, "export", &["Ctrl+Shift+S"]);
    add(&mut b, "copy", &["Ctrl+Shift+C"]);
    add(&mut b, "paste", &["Ctrl+Shift+V", "Shift+Ins"]);

    // Atajos cambiados por el usuario (KXMLGUI): <Action name="…" shortcut="A; B"/>
    let Some(base) = home_dir().map(|h| h.join(".local/share/kxmlgui5/konsole")) else { return b };
    for file in ["konsoleui.rc", "sessionui.rc"] {
        let Ok(text) = fs::read_to_string(base.join(file)) else { continue };
        for tag in text.split("<Action ").skip(1) {
            let tag = tag.split('>').next().unwrap_or("");
            let (Some(name), Some(shortcut)) = (xml_attr(tag, "name"), xml_attr(tag, "shortcut")) else {
                continue;
            };
            let num = name.strip_prefix("switch-to-tab-").and_then(|n| n.parse::<usize>().ok());
            let action = match num {
                Some(n) => format!("goto_tab_{}", n + 1),
                None => match KONSOLE_ACTIONS.iter().find(|(k, _)| *k == name) {
                    Some((_, a)) => a.to_string(),
                    None => continue,
                },
            };
            // close-session y close-active-view comparten acción: se suman.
            let shared = KONSOLE_ACTIONS.iter().filter(|(_, a)| *a == action).count() > 1;
            let keys: Vec<String> = shortcut.split(';').filter_map(|k| parse_plus(k, &[])).collect();
            let entry = b.entry(action).or_default();
            if shared {
                entry.extend(keys);
            } else {
                *entry = keys;
            }
        }
    }
    b
}

fn xml_attr(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">"))
}

// ── GNOME Terminal ──────────────────────────────────────────────────

fn gnome_terminal() -> Binds {
    let mut b = Binds::new();
    let schema = "org.gnome.Terminal.Legacy.Keybindings:/org/gnome/terminal/legacy/keybindings/";
    let Some(list) = gsettings(&["list-recursively", schema]) else { return b };
    for line in list.lines() {
        // "<schema> <clave> '<valor>'"
        let mut parts = line.splitn(3, ' ');
        let (_, Some(key), Some(value)) = (parts.next(), parts.next(), parts.next()) else { continue };
        let num = key.strip_prefix("switch-to-tab-").and_then(|n| n.parse::<usize>().ok());
        let action = match (key, num) {
            (_, Some(n)) => format!("goto_tab_{n}"),
            ("new-tab", _) => "new_tab".into(),
            ("new-window", _) => "new_window".into(),
            ("close-tab", _) => "close_pane".into(),
            ("next-tab", _) => "next_tab".into(),
            ("prev-tab", _) => "prev_tab".into(),
            ("move-tab-left", _) => "move_tab_left".into(),
            ("move-tab-right", _) => "move_tab_right".into(),
            ("zoom-in", _) => "zoom_in".into(),
            ("zoom-out", _) => "zoom_out".into(),
            ("zoom-normal", _) => "zoom_reset".into(),
            ("save-contents", _) => "export".into(),
            ("copy", _) => "copy".into(),
            ("paste", _) => "paste".into(),
            _ => continue,
        };
        b.insert(action, parse_gtk(value).into_iter().collect());
    }
    b
}

// ── kitty ───────────────────────────────────────────────────────────

fn kitty_action(cmd: &str) -> Option<String> {
    let mut words = cmd.split_whitespace();
    let name = words.next()?;
    let args: Vec<&str> = words.collect();
    Some(
        match name {
            "new_tab" | "new_tab_with_cwd" => "new_tab",
            "close_tab" => "close_tab",
            "next_tab" => "next_tab",
            "previous_tab" => "prev_tab",
            "move_tab_backward" => "move_tab_left",
            "move_tab_forward" => "move_tab_right",
            "goto_tab" => return args.first().and_then(|n| n.parse::<usize>().ok()).map(|n| format!("goto_tab_{n}")),
            "new_window" | "new_window_with_cwd" => "split_right",
            "launch" if args.contains(&"--location=vsplit") => "split_right",
            "launch" if args.contains(&"--location=hsplit") => "split_down",
            "close_window" => "close_pane",
            "next_window" => "next_pane",
            "previous_window" => "prev_pane",
            "neighboring_window" => match args.first().copied()? {
                "left" => "focus_left",
                "right" => "focus_right",
                "up" | "top" => "focus_up",
                "down" | "bottom" => "focus_down",
                _ => return None,
            },
            "new_os_window" | "new_os_window_with_cwd" => "new_window",
            "toggle_layout" if args.first() == Some(&"stack") => "maximize_pane",
            "change_font_size" => match args.get(1).copied()? {
                a if a.starts_with('+') => "zoom_in",
                a if a.starts_with('-') => "zoom_out",
                _ => "zoom_reset",
            },
            "copy_to_clipboard" => "copy",
            "paste_from_clipboard" => "paste",
            _ => return None,
        }
        .to_string(),
    )
}

fn kitty_maps(path: &Path, out: &mut Vec<(String, String)>, kitty_mod: &mut String, clear: &mut bool, depth: u8) {
    let Ok(text) = fs::read_to_string(path) else { return };
    for line in text.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once(char::is_whitespace) else { continue };
        let value = value.trim();
        match key {
            "include" if depth < 3 => {
                let inc = path.parent().unwrap_or(Path::new(".")).join(value);
                kitty_maps(&inc, out, kitty_mod, clear, depth + 1);
            }
            "kitty_mod" => *kitty_mod = value.to_string(),
            "clear_all_shortcuts" => *clear = value == "yes",
            "map" => {
                if let Some((keys, cmd)) = value.split_once(char::is_whitespace) {
                    out.push((keys.to_string(), cmd.trim().to_string()));
                }
            }
            _ => {}
        }
    }
}

fn kitty() -> Binds {
    let mut maps = vec![];
    let mut kitty_mod = "ctrl+shift".to_string();
    let mut clear = false;
    if let Some(path) = kitty_conf() {
        kitty_maps(&path, &mut maps, &mut kitty_mod, &mut clear, 0);
    }
    let km: Vec<&'static str> = kitty_mod.split('+').filter_map(|m| modifier(&m.to_lowercase())).collect();
    let km = if km.is_empty() { vec!["ctrl", "shift"] } else { km };

    let mut b = Binds::new();
    let put = |b: &mut Binds, keys: &str, cmd: &str| {
        let Some(k) = parse_plus(keys, &km) else { return };
        // Un atajo nuevo pisa al que tuviera esa combinación.
        for v in b.values_mut() {
            v.retain(|x| x != &k);
        }
        if let Some(a) = kitty_action(cmd) {
            b.entry(a).or_default().push(k);
        }
    };
    if !clear {
        let defaults = [
            ("kitty_mod+t", "new_tab"),
            ("kitty_mod+q", "close_tab"),
            ("kitty_mod+right", "next_tab"),
            ("ctrl+tab", "next_tab"),
            ("kitty_mod+left", "previous_tab"),
            ("ctrl+shift+tab", "previous_tab"),
            ("kitty_mod+.", "move_tab_forward"),
            ("kitty_mod+,", "move_tab_backward"),
            ("kitty_mod+enter", "new_window"),
            ("kitty_mod+w", "close_window"),
            ("kitty_mod+]", "next_window"),
            ("kitty_mod+[", "previous_window"),
            ("kitty_mod+n", "new_os_window"),
            ("kitty_mod+equal", "change_font_size all +2.0"),
            ("kitty_mod+plus", "change_font_size all +2.0"),
            ("kitty_mod+minus", "change_font_size all -2.0"),
            ("kitty_mod+backspace", "change_font_size all 0"),
            ("kitty_mod+c", "copy_to_clipboard"),
            ("kitty_mod+v", "paste_from_clipboard"),
        ];
        for (k, c) in defaults {
            put(&mut b, k, c);
        }
        // En macOS kitty suma los atajos con cmd.
        if cfg!(target_os = "macos") {
            let mac = [
                ("cmd+t", "new_tab".to_string()),
                ("cmd+w", "close_tab".into()),
                ("shift+cmd+]", "next_tab".into()),
                ("shift+cmd+[", "previous_tab".into()),
                ("cmd+enter", "new_window".into()),
                ("shift+cmd+d", "close_window".into()),
                ("cmd+n", "new_os_window".into()),
                ("cmd+plus", "change_font_size all +2.0".into()),
                ("cmd+equal", "change_font_size all +2.0".into()),
                ("cmd+minus", "change_font_size all -2.0".into()),
                ("cmd+0", "change_font_size all 0".into()),
                ("cmd+c", "copy_to_clipboard".into()),
                ("cmd+v", "paste_from_clipboard".into()),
            ];
            for (k, c) in &mac {
                put(&mut b, k, c);
            }
            for i in 1..=9 {
                put(&mut b, &format!("cmd+{i}"), &format!("goto_tab {i}"));
            }
        }
    }
    for (k, c) in &maps {
        put(&mut b, k, c);
    }
    b
}

// ── Alacritty ───────────────────────────────────────────────────────

fn alacritty() -> Binds {
    let mut b = Binds::new();
    if cfg!(target_os = "macos") {
        add(&mut b, "new_window", &["cmd+n"]);
        add(&mut b, "zoom_in", &["cmd+=", "cmd++"]);
        add(&mut b, "zoom_out", &["cmd+-"]);
        add(&mut b, "zoom_reset", &["cmd+0"]);
        add(&mut b, "copy", &["cmd+c"]);
        add(&mut b, "paste", &["cmd+v"]);
    } else {
        add(&mut b, "zoom_in", &["ctrl+=", "ctrl++"]);
        add(&mut b, "zoom_out", &["ctrl+-"]);
        add(&mut b, "zoom_reset", &["ctrl+0"]);
        add(&mut b, "copy", &["ctrl+shift+c"]);
        add(&mut b, "paste", &["ctrl+shift+v"]);
    }
    let Some(path) = alacritty_conf() else { return b };
    let Some(doc) = fs::read_to_string(path).ok().and_then(|t| t.parse::<toml::Table>().ok()) else {
        return b;
    };
    let list = doc
        .get("keyboard")
        .and_then(|k| k.get("bindings"))
        .and_then(|l| l.as_array())
        .cloned()
        .unwrap_or_default();
    for entry in list {
        let (Some(key), Some(action)) = (
            entry.get("key").and_then(|k| k.as_str()),
            entry.get("action").and_then(|a| a.as_str()),
        ) else {
            continue;
        };
        let action = match action.to_lowercase().as_str() {
            "createnewwindow" | "spawnnewinstance" => "new_window",
            "increasefontsize" => "zoom_in",
            "decreasefontsize" => "zoom_out",
            "resetfontsize" => "zoom_reset",
            "copy" => "copy",
            "paste" => "paste",
            _ => continue,
        };
        let mods: Vec<&str> = entry
            .get("mods")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .split('|')
            .filter_map(|m| modifier(&m.trim().to_lowercase()))
            .collect();
        if let Some(k) = accel(&mods, key) {
            b.entry(action.into()).or_default().push(k);
        }
    }
    b
}

// ── Ghostty ─────────────────────────────────────────────────────────

fn ghostty_action(cmd: &str) -> Option<String> {
    let (name, arg) = cmd.split_once(':').unwrap_or((cmd, ""));
    Some(
        match name.trim() {
            "new_tab" => "new_tab",
            "close_tab" => "close_tab",
            "close_surface" => "close_pane",
            "next_tab" => "next_tab",
            "previous_tab" => "prev_tab",
            "move_tab" if arg.starts_with('-') => "move_tab_left",
            "move_tab" => "move_tab_right",
            "goto_tab" => return arg.trim().parse::<usize>().ok().map(|n| format!("goto_tab_{n}")),
            "new_split" => match arg.trim() {
                "down" | "up" => "split_down",
                _ => "split_right",
            },
            "goto_split" => match arg.trim() {
                "left" => "focus_left",
                "right" => "focus_right",
                "up" | "top" => "focus_up",
                "down" | "bottom" => "focus_down",
                "next" => "next_pane",
                "previous" => "prev_pane",
                _ => return None,
            },
            "toggle_split_zoom" => "maximize_pane",
            "new_window" => "new_window",
            "increase_font_size" => "zoom_in",
            "decrease_font_size" => "zoom_out",
            "reset_font_size" => "zoom_reset",
            "copy_to_clipboard" => "copy",
            "paste_from_clipboard" => "paste",
            _ => return None,
        }
        .to_string(),
    )
}

fn ghostty() -> Binds {
    let mut b = Binds::new();
    let put = |b: &mut Binds, keys: &str, cmd: &str| {
        let keys = keys.trim().trim_start_matches("global:").trim_start_matches("all:");
        let Some(k) = parse_plus(keys, &[]) else { return };
        for v in b.values_mut() {
            v.retain(|x| x != &k);
        }
        if let Some(a) = ghostty_action(cmd) {
            b.entry(a).or_default().push(k);
        }
    };
    let mut defaults: Vec<(String, String)> = if cfg!(target_os = "macos") {
        [
            ("cmd+t", "new_tab"),
            ("cmd+w", "close_surface"),
            ("cmd+n", "new_window"),
            ("ctrl+tab", "next_tab"),
            ("cmd+shift+]", "next_tab"),
            ("ctrl+shift+tab", "previous_tab"),
            ("cmd+shift+[", "previous_tab"),
            ("cmd+d", "new_split:right"),
            ("cmd+shift+d", "new_split:down"),
            ("cmd+alt+left", "goto_split:left"),
            ("cmd+alt+right", "goto_split:right"),
            ("cmd+alt+up", "goto_split:up"),
            ("cmd+alt+down", "goto_split:down"),
            ("cmd+[", "goto_split:previous"),
            ("cmd+]", "goto_split:next"),
            ("cmd+shift+enter", "toggle_split_zoom"),
            ("cmd+equal", "increase_font_size:1"),
            ("cmd+plus", "increase_font_size:1"),
            ("cmd+minus", "decrease_font_size:1"),
            ("cmd+0", "reset_font_size"),
            ("cmd+c", "copy_to_clipboard"),
            ("cmd+v", "paste_from_clipboard"),
        ]
        .iter()
        .map(|(k, c)| (k.to_string(), c.to_string()))
        .collect()
    } else {
        [
            ("ctrl+shift+t", "new_tab"),
            ("ctrl+shift+w", "close_surface"),
            ("ctrl+tab", "next_tab"),
            ("ctrl+page_down", "next_tab"),
            ("ctrl+shift+right", "next_tab"),
            ("ctrl+shift+tab", "previous_tab"),
            ("ctrl+page_up", "previous_tab"),
            ("ctrl+shift+left", "previous_tab"),
            ("ctrl+shift+o", "new_split:right"),
            ("ctrl+shift+e", "new_split:down"),
            ("ctrl+alt+left", "goto_split:left"),
            ("ctrl+alt+right", "goto_split:right"),
            ("ctrl+alt+up", "goto_split:up"),
            ("ctrl+alt+down", "goto_split:down"),
            ("ctrl+super+[", "goto_split:previous"),
            ("ctrl+super+]", "goto_split:next"),
            ("ctrl+shift+enter", "toggle_split_zoom"),
            ("ctrl+shift+n", "new_window"),
            ("ctrl+equal", "increase_font_size:1"),
            ("ctrl+plus", "increase_font_size:1"),
            ("ctrl+minus", "decrease_font_size:1"),
            ("ctrl+0", "reset_font_size"),
            ("ctrl+shift+c", "copy_to_clipboard"),
            ("ctrl+shift+v", "paste_from_clipboard"),
        ]
        .iter()
        .map(|(k, c)| (k.to_string(), c.to_string()))
        .collect()
    };
    let digit = if cfg!(target_os = "macos") { "cmd" } else { "alt" };
    for i in 1..=8 {
        defaults.push((format!("{digit}+{i}"), format!("goto_tab:{i}")));
    }
    let user = ghostty_conf().and_then(|p| fs::read_to_string(p).ok()).unwrap_or_default();
    let binds: Vec<&str> = user
        .lines()
        .filter_map(|l| l.split_once('=').filter(|(k, _)| k.trim() == "keybind").map(|(_, v)| v.trim()))
        .collect();
    if !binds.contains(&"clear") {
        for (k, c) in &defaults {
            put(&mut b, k, c);
        }
    }
    for v in binds {
        if v == "clear" {
            b.clear();
        } else if let Some((k, c)) = v.split_once('=') {
            put(&mut b, k, c);
        }
    }
    b
}

// ── foot ────────────────────────────────────────────────────────────

fn foot() -> Binds {
    let mut b = Binds::new();
    add(&mut b, "new_window", &["ctrl+shift+n"]);
    add(&mut b, "zoom_in", &["ctrl++", "ctrl+="]);
    add(&mut b, "zoom_out", &["ctrl+-"]);
    add(&mut b, "zoom_reset", &["ctrl+0"]);
    add(&mut b, "copy", &["ctrl+shift+c"]);
    add(&mut b, "paste", &["ctrl+shift+v"]);
    let Some(text) = config_home().and_then(|c| fs::read_to_string(c.join("foot/foot.ini")).ok()) else {
        return b;
    };
    for (section, key, value) in crate::terminal::parse_ini(&text) {
        if section != "key-bindings" {
            continue;
        }
        let action = match key.as_str() {
            "spawn-terminal" => "new_window",
            "font-increase" => "zoom_in",
            "font-decrease" => "zoom_out",
            "font-reset" => "zoom_reset",
            "clipboard-copy" => "copy",
            "clipboard-paste" => "paste",
            _ => continue,
        };
        let keys = if value == "none" {
            vec![]
        } else {
            value.split_whitespace().filter_map(|k| parse_plus(k, &[])).collect()
        };
        b.insert(action.into(), keys);
    }
    b
}

// ── Windows Terminal ────────────────────────────────────────────────

fn wt_action(cmd: &serde_json::Value) -> Option<String> {
    let (name, obj) = match cmd {
        serde_json::Value::String(s) => (s.as_str(), None),
        serde_json::Value::Object(o) => (o.get("action")?.as_str()?, Some(o)),
        _ => return None,
    };
    let arg = |k: &str| obj.and_then(|o| o.get(k)).and_then(|v| v.as_str()).unwrap_or("");
    Some(
        match name {
            "newTab" => "new_tab",
            "closePane" => "close_pane",
            "closeTab" => "close_tab",
            "nextTab" => "next_tab",
            "prevTab" => "prev_tab",
            "moveTab" if arg("direction") == "backward" => "move_tab_left",
            "moveTab" => "move_tab_right",
            "switchToTab" => {
                let i = obj.and_then(|o| o.get("index")).and_then(|v| v.as_u64())?;
                return Some(format!("goto_tab_{}", i + 1));
            }
            "splitPane" => match arg("split") {
                "down" | "up" | "horizontal" => "split_down",
                _ => "split_right",
            },
            "moveFocus" => match arg("direction") {
                "left" => "focus_left",
                "right" => "focus_right",
                "up" => "focus_up",
                "down" => "focus_down",
                "nextInOrder" => "next_pane",
                "previousInOrder" => "prev_pane",
                _ => return None,
            },
            "togglePaneZoom" => "maximize_pane",
            "newWindow" => "new_window",
            "adjustFontSize" => {
                let d = obj.and_then(|o| o.get("delta")).and_then(|v| v.as_f64()).unwrap_or(1.0);
                if d < 0.0 { "zoom_out" } else { "zoom_in" }
            }
            "resetFontSize" => "zoom_reset",
            "copy" => "copy",
            "paste" => "paste",
            _ => return None,
        }
        .to_string(),
    )
}

fn windows_terminal() -> Binds {
    let mut b = Binds::new();
    add(&mut b, "new_tab", &["ctrl+shift+t"]);
    add(&mut b, "close_pane", &["ctrl+shift+w"]);
    add(&mut b, "new_window", &["ctrl+shift+n"]);
    add(&mut b, "next_tab", &["ctrl+tab"]);
    add(&mut b, "prev_tab", &["ctrl+shift+tab"]);
    digits(&mut b, "ctrl+alt+", 9);
    add(&mut b, "split_right", &["alt+shift++", "alt+shift+="]);
    add(&mut b, "split_down", &["alt+shift+-"]);
    add(&mut b, "focus_left", &["alt+left"]);
    add(&mut b, "focus_right", &["alt+right"]);
    add(&mut b, "focus_up", &["alt+up"]);
    add(&mut b, "focus_down", &["alt+down"]);
    add(&mut b, "zoom_in", &["ctrl+="]);
    add(&mut b, "zoom_out", &["ctrl+-"]);
    add(&mut b, "zoom_reset", &["ctrl+0"]);
    add(&mut b, "copy", &["ctrl+shift+c"]);
    add(&mut b, "paste", &["ctrl+shift+v", "ctrl+v"]);

    let Some(local) = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from) else { return b };
    let Some(text) = [
        "Packages/Microsoft.WindowsTerminal_8wekyb3d8bbwe/LocalState/settings.json",
        "Packages/Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe/LocalState/settings.json",
        "Microsoft/Windows Terminal/settings.json",
    ]
    .iter()
    .find_map(|p| fs::read_to_string(local.join(p)).ok()) else {
        return b;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&strip_jsonc(&text)) else { return b };
    let arr = |k: &str| v.get(k).and_then(|a| a.as_array()).cloned().unwrap_or_default();
    // Formato nuevo: "actions" [{command, id}] + "keybindings" [{id, keys}].
    let by_id: HashMap<String, serde_json::Value> = arr("actions")
        .into_iter()
        .filter_map(|a| Some((a.get("id")?.as_str()?.to_string(), a.get("command")?.clone())))
        .collect();
    let mut entries: Vec<(serde_json::Value, Option<serde_json::Value>)> = arr("actions")
        .into_iter()
        .chain(arr("keybindings"))
        .filter_map(|e| {
            let keys = e.get("keys")?.clone();
            let cmd = e
                .get("command")
                .cloned()
                .or_else(|| e.get("id").and_then(|i| i.as_str()).and_then(|i| by_id.get(i).cloned()));
            Some((keys, cmd))
        })
        .collect();
    for (keys, cmd) in entries.drain(..) {
        let list: Vec<String> = match keys {
            serde_json::Value::String(s) => vec![s],
            serde_json::Value::Array(a) => a.iter().filter_map(|k| k.as_str().map(String::from)).collect(),
            _ => continue,
        };
        for k in list.iter().filter_map(|k| parse_plus(k, &[])) {
            for v in b.values_mut() {
                v.retain(|x| x != &k);
            }
            if let Some(a) = cmd.as_ref().and_then(wt_action) {
                b.entry(a).or_default().push(k);
            }
        }
    }
    b
}

// ── macOS: iTerm2 y Terminal.app ────────────────────────────────────

/// "\U2192" → "→" (así escribe `defaults read` lo que no es ASCII).
fn unescape_defaults(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("\\U") {
        out.push_str(&rest[..i]);
        let hex = rest.get(i + 2..i + 6).unwrap_or("");
        match u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
            Some(c) if hex.len() == 4 => {
                out.push(c);
                rest = &rest[i + 6..];
            }
            _ => {
                out.push_str("\\U");
                rest = &rest[i + 2..];
            }
        }
    }
    out.push_str(rest);
    out.replace("\\\"", "\"").replace("\\\\", "\\")
}

/// Equivalente de teclado de Cocoa: "@~d" = cmd+alt+d ("@" cmd, "~" alt,
/// "$" shift, "^" ctrl). Las flechas usan los caracteres de función de AppKit.
fn parse_cocoa(s: &str) -> Option<String> {
    let mut mods = vec![];
    let mut chars = s.chars().peekable();
    while let Some(&c) = chars.peek() {
        let m = match c {
            '@' => "meta",
            '~' => "alt",
            '$' => "shift",
            '^' => "ctrl",
            _ => break,
        };
        // Un modificador solo es la tecla si es el último carácter ("@$" no existe; "@~" sí sería ~).
        let mut ahead = chars.clone();
        ahead.next();
        if ahead.peek().is_none() {
            break;
        }
        mods.push(m);
        chars.next();
    }
    let key: String = chars.collect();
    let key = match key.as_str() {
        "\u{F700}" | "\u{2191}" => "up".to_string(),
        "\u{F701}" | "\u{2193}" => "down".to_string(),
        "\u{F702}" | "\u{2190}" => "left".to_string(),
        "\u{F703}" | "\u{2192}" => "right".to_string(),
        "\u{F72C}" => "pageup".to_string(),
        "\u{F72D}" => "pagedown".to_string(),
        "\r" | "\n" | "\u{21A9}" => "enter".to_string(),
        "\t" | "\u{21E5}" => "tab".to_string(),
        k if k.chars().count() == 1 => {
            let c = k.chars().next()?;
            // Una mayúscula implica shift.
            if c.is_uppercase() && !mods.contains(&"shift") {
                mods.push("shift");
            }
            c.to_lowercase().to_string()
        }
        _ => return None,
    };
    accel(&mods, &key)
}

/// Atajos de menú que el usuario cambió en Ajustes del Sistema → Teclado →
/// Atajos de apps (`NSUserKeyEquivalents`), globales y de la app: título → atajo.
fn mac_user_shortcuts(domain: &str) -> Vec<(String, Option<String>)> {
    let mut out = vec![];
    for d in ["NSGlobalDomain", domain] {
        let Ok(o) = std::process::Command::new("defaults").args(["read", d, "NSUserKeyEquivalents"]).output() else {
            continue;
        };
        if !o.status.success() {
            continue;
        }
        // { "New Tab" = "@t"; NewWindow = "@n"; }
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            let line = line.trim().trim_end_matches(';');
            let Some((title, value)) = line.split_once(" = ") else { continue };
            let title = unescape_defaults(title.trim().trim_matches('"'));
            let value = unescape_defaults(value.trim().trim_matches('"'));
            // Un atajo vacío desactiva el del menú.
            out.push((title, if value.is_empty() { None } else { parse_cocoa(&value) }));
        }
    }
    out
}

fn apply_mac_shortcuts(b: &mut Binds, domain: &str, titles: &[(&str, &str)]) {
    for (title, key) in mac_user_shortcuts(domain) {
        let Some((_, action)) = titles.iter().find(|(t, _)| t.eq_ignore_ascii_case(&title)) else { continue };
        if let Some(k) = &key {
            for v in b.values_mut() {
                v.retain(|x| x != k);
            }
        }
        b.insert(action.to_string(), key.into_iter().collect());
    }
}

fn iterm2() -> Binds {
    let mut b = Binds::new();
    add(&mut b, "new_tab", &["cmd+t"]);
    add(&mut b, "new_window", &["cmd+n"]);
    add(&mut b, "close_pane", &["cmd+w"]);
    add(&mut b, "next_tab", &["cmd+shift+]", "ctrl+tab"]);
    add(&mut b, "prev_tab", &["cmd+shift+[", "ctrl+shift+tab"]);
    digits(&mut b, "cmd+", 9);
    add(&mut b, "split_right", &["cmd+d"]);
    add(&mut b, "split_down", &["cmd+shift+d"]);
    add(&mut b, "focus_left", &["cmd+alt+left"]);
    add(&mut b, "focus_right", &["cmd+alt+right"]);
    add(&mut b, "focus_up", &["cmd+alt+up"]);
    add(&mut b, "focus_down", &["cmd+alt+down"]);
    add(&mut b, "next_pane", &["cmd+]"]);
    add(&mut b, "prev_pane", &["cmd+["]);
    add(&mut b, "maximize_pane", &["cmd+shift+enter"]);
    add(&mut b, "zoom_in", &["cmd++", "cmd+="]);
    add(&mut b, "zoom_out", &["cmd+-"]);
    add(&mut b, "zoom_reset", &["cmd+0"]);
    add(&mut b, "copy", &["cmd+c"]);
    add(&mut b, "paste", &["cmd+v"]);
    apply_mac_shortcuts(
        &mut b,
        "com.googlecode.iterm2",
        &[
            ("New Tab", "new_tab"),
            ("New Window", "new_window"),
            ("Close", "close_pane"),
            ("Close Tab", "close_tab"),
            ("Select Next Tab", "next_tab"),
            ("Select Previous Tab", "prev_tab"),
            ("Move Tab Left", "move_tab_left"),
            ("Move Tab Right", "move_tab_right"),
            ("Split Vertically with Current Profile", "split_right"),
            ("Split Horizontally with Current Profile", "split_down"),
            ("Select Pane Left", "focus_left"),
            ("Select Pane Right", "focus_right"),
            ("Select Pane Above", "focus_up"),
            ("Select Pane Below", "focus_down"),
            ("Select Next Pane", "next_pane"),
            ("Select Previous Pane", "prev_pane"),
            ("Maximize Active Pane", "maximize_pane"),
            ("Make Text Bigger", "zoom_in"),
            ("Make Text Smaller", "zoom_out"),
            ("Make Text Normal Size", "zoom_reset"),
            ("Save Contents…", "export"),
            ("Copy", "copy"),
            ("Paste", "paste"),
        ],
    );
    b
}

fn terminal_app() -> Binds {
    let mut b = Binds::new();
    add(&mut b, "new_tab", &["cmd+t"]);
    add(&mut b, "new_window", &["cmd+n"]);
    add(&mut b, "close_pane", &["cmd+w"]);
    add(&mut b, "next_tab", &["cmd+shift+]", "ctrl+tab"]);
    add(&mut b, "prev_tab", &["cmd+shift+[", "ctrl+shift+tab"]);
    digits(&mut b, "cmd+", 9);
    add(&mut b, "zoom_in", &["cmd++", "cmd+="]);
    add(&mut b, "zoom_out", &["cmd+-"]);
    add(&mut b, "zoom_reset", &["cmd+0"]);
    add(&mut b, "export", &["cmd+s"]);
    add(&mut b, "copy", &["cmd+c"]);
    add(&mut b, "paste", &["cmd+v"]);
    apply_mac_shortcuts(
        &mut b,
        "com.apple.Terminal",
        &[
            ("New Tab", "new_tab"),
            ("New Window", "new_window"),
            ("Close Tab", "close_pane"),
            ("Show Next Tab", "next_tab"),
            ("Show Previous Tab", "prev_tab"),
            ("Move Tab to New Window", "new_window"),
            ("Bigger", "zoom_in"),
            ("Smaller", "zoom_out"),
            ("Default Font Size", "zoom_reset"),
            ("Export Text As…", "export"),
            ("Copy", "copy"),
            ("Paste", "paste"),
        ],
    );
    b
}

// ── Selección ───────────────────────────────────────────────────────

fn terminal_binds(name: &str) -> Option<Binds> {
    Some(match name {
        "konsole" => konsole(),
        "gnome-terminal" => gnome_terminal(),
        "kitty" => kitty(),
        "alacritty" => alacritty(),
        "ghostty" => ghostty(),
        "foot" => foot(),
        "windows-terminal" => windows_terminal(),
        "iterm2" => iterm2(),
        "terminal-app" => terminal_app(),
        _ => return None,
    })
}

/// `which`: "auto", "off" o el nombre de una terminal, como `terminal_style`.
#[tauri::command]
pub async fn terminal_keys(which: String) -> Keymap {
    let name = match which.as_str() {
        "off" => None,
        "auto" => auto_order().into_iter().find(|t| read(t).is_some()).map(String::from),
        t => read(t).map(|_| t.to_string()),
    };
    let mut binds = name.as_deref().and_then(terminal_binds).unwrap_or_default();
    // Lo que la terminal no tiene (p. ej. divisiones en GNOME Terminal) sale
    // de los atajos del sistema, sin quitarle combinaciones a la terminal.
    let used: Vec<String> = binds.values().flatten().cloned().collect();
    for (action, keys) in os_defaults() {
        let entry = binds.entry(action).or_default();
        if entry.is_empty() {
            entry.extend(keys.into_iter().filter(|k| !used.contains(k)));
        }
    }
    let bindings = binds
        .into_iter()
        .map(|(a, mut k)| {
            k.dedup();
            (a, k)
        })
        .filter(|(_, k)| !k.is_empty())
        .collect();
    Keymap { terminal: name, bindings }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_accelerators() {
        assert_eq!(parse_plus("Ctrl+Shift+T", &[]).as_deref(), Some("ctrl+shift+t"));
        assert_eq!(parse_plus("Ctrl++", &[]).as_deref(), Some("ctrl++"));
        assert_eq!(parse_plus("Ctrl+(", &[]).as_deref(), Some("ctrl+("));
        assert_eq!(parse_plus("Shift+Right", &[]).as_deref(), Some("shift+right"));
        assert_eq!(parse_plus("Ctrl+PgDown", &[]).as_deref(), Some("ctrl+pagedown"));
        assert_eq!(parse_plus("kitty_mod+enter", &["ctrl", "shift"]).as_deref(), Some("ctrl+shift+enter"));
        assert_eq!(parse_plus("super+ctrl+]", &[]).as_deref(), Some("ctrl+meta+]"));
        assert_eq!(parse_gtk("'<Primary><Shift>t'").as_deref(), Some("ctrl+shift+t"));
        assert_eq!(parse_gtk("<Control>Page_Down").as_deref(), Some("ctrl+pagedown"));
        assert_eq!(parse_gtk("disabled"), None);
        assert_eq!(parse_cocoa("@~d").as_deref(), Some("alt+meta+d"));
        assert_eq!(parse_cocoa("@$]").as_deref(), Some("shift+meta+]"));
        assert_eq!(parse_cocoa("@T").as_deref(), Some("shift+meta+t"));
        assert_eq!(parse_cocoa("@~\u{F702}").as_deref(), Some("alt+meta+left"));
        assert_eq!(parse_cocoa(&unescape_defaults("@~\\UF702")).as_deref(), Some("alt+meta+left"));
        assert_eq!(parse_cocoa("@~").as_deref(), Some("meta+~"));
    }

    #[test]
    fn konsole_overrides() {
        let tag = r#"Action shortcut="Ctrl+T; Ctrl+Shift+T" name="new-tab"/"#;
        assert_eq!(xml_attr(tag, "name").as_deref(), Some("new-tab"));
        assert_eq!(xml_attr(tag, "shortcut").as_deref(), Some("Ctrl+T; Ctrl+Shift+T"));
    }
}
