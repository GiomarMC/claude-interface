import { invoke } from "@tauri-apps/api/core";

/** Acciones de ventana que se manejan con los atajos de la terminal del usuario. */
export type Action =
  | "new_tab"
  | "close_tab"
  | "close_pane"
  | "new_window"
  | "next_tab"
  | "prev_tab"
  | "move_tab_left"
  | "move_tab_right"
  | `goto_tab_${number}`
  | "split_right"
  | "split_down"
  | "focus_left"
  | "focus_right"
  | "focus_up"
  | "focus_down"
  | "next_pane"
  | "prev_pane"
  | "maximize_pane"
  | "zoom_in"
  | "zoom_out"
  | "zoom_reset"
  | "export"
  | "copy";

export const ACTION_LABELS: [Action, string][] = [
  ["new_tab", "nueva pestaña"],
  ["close_pane", "cerrar panel"],
  ["close_tab", "cerrar pestaña"],
  ["next_tab", "pestaña siguiente"],
  ["prev_tab", "pestaña anterior"],
  ["move_tab_left", "mover pestaña a la izquierda"],
  ["move_tab_right", "mover pestaña a la derecha"],
  ["goto_tab_1", "ir a la pestaña 1 (…9)"],
  ["split_right", "dividir izquierda/derecha"],
  ["split_down", "dividir arriba/abajo"],
  ["focus_left", "panel de la izquierda"],
  ["focus_right", "panel de la derecha"],
  ["focus_up", "panel de arriba"],
  ["focus_down", "panel de abajo"],
  ["next_pane", "panel siguiente"],
  ["prev_pane", "panel anterior"],
  ["maximize_pane", "maximizar panel"],
  ["new_window", "nueva ventana"],
  ["zoom_in", "agrandar fuente"],
  ["zoom_out", "achicar fuente"],
  ["zoom_reset", "tamaño de fuente original"],
  ["export", "exportar a .md"],
  ["copy", "copiar selección"],
];

const NAMED: Record<string, string> = {
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  ArrowDown: "down",
  PageUp: "pageup",
  PageDown: "pagedown",
  " ": "space",
};

/** Tecla física (distribución US), para atajos como ctrl+shift+] en otros teclados. */
function codeKey(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3).toLowerCase();
  if (/^Digit\d$/.test(code)) return code.slice(5);
  const map: Record<string, string> = {
    BracketLeft: "[",
    BracketRight: "]",
    Backslash: "\\",
    Minus: "-",
    Equal: "=",
    Comma: ",",
    Period: ".",
    Slash: "/",
    Semicolon: ";",
    Quote: "'",
    Backquote: "`",
  };
  return map[code] ?? null;
}

/** Formas normalizadas ("ctrl+shift+t") en las que puede venir escrito el atajo de este evento. */
export function eventAccels(e: KeyboardEvent): string[] {
  const mods = (shift: boolean) =>
    [e.ctrlKey && "ctrl", e.altKey && "alt", shift && "shift", e.metaKey && "meta"].filter(Boolean).join("+");
  const join = (m: string, k: string) => (m ? `${m}+${k}` : k);
  const out = new Set<string>();
  const key = e.key.length === 1 ? e.key.toLowerCase() : (NAMED[e.key] ?? e.key.toLowerCase());
  // "(" ya implica shift: la terminal lo escribe "ctrl+(".
  if (e.key.length === 1 && !/\p{L}/u.test(e.key)) out.add(join(mods(false), key));
  out.add(join(mods(e.shiftKey), key));
  const physical = codeKey(e.code);
  if (physical) out.add(join(mods(e.shiftKey), physical));
  return [...out];
}

const IS_MAC = /Mac/.test(navigator.platform || navigator.userAgent);

/** "meta" es cmd en macOS y la tecla Windows/Super en el resto. */
export function displayAccel(accel: string): string {
  return accel.replace(/\bmeta\b/, IS_MAC ? "cmd" : "super").replace(/\balt\b/, IS_MAC ? "opt" : "alt");
}

class Keymap {
  terminal = $state<string | null>(null);
  bindings = $state<Partial<Record<Action, string[]>>>({});
  private byAccel = new Map<string, Action>();

  async load(which: string) {
    try {
      const k = await invoke<{ terminal: string | null; bindings: Record<Action, string[]> }>("terminal_keys", { which });
      this.terminal = k?.terminal ?? null;
      this.bindings = k?.bindings ?? {};
    } catch {
      this.bindings = {};
    }
    this.byAccel.clear();
    for (const [action, accels] of Object.entries(this.bindings) as [Action, string[]][]) {
      for (const a of accels) if (!this.byAccel.has(a)) this.byAccel.set(a, action);
    }
  }

  match(e: KeyboardEvent): Action | null {
    for (const a of eventAccels(e)) {
      const action = this.byAccel.get(a);
      if (action) return action;
    }
    return null;
  }

  /** Primer atajo de una acción, para mostrarlo ("ctrl+shift+t"). */
  label(action: Action): string | null {
    const k = this.bindings[action]?.[0];
    return k ? displayAccel(k) : null;
  }
}

export const keymap = new Keymap();
