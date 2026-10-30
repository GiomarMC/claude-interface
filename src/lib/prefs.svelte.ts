import { invoke } from "@tauri-apps/api/core";
import { setPalette } from "./ansi";

/** Temas de Claude Code (`/theme`) más "auto", que sigue al sistema. */
export const THEMES = [
  "dark",
  "light",
  "dark-daltonized",
  "light-daltonized",
  "dark-ansi",
  "light-ansi",
  "auto",
] as const;

export const TERMINALS = [
  "konsole",
  "gnome-terminal",
  "kitty",
  "alacritty",
  "ghostty",
  "foot",
  "windows-terminal",
  "iterm2",
  "terminal-app",
] as const;

export const DEFAULT_FONT_SIZE = 14;

export type TerminalStyle = {
  terminal: string;
  profile?: string | null;
  fontFamily?: string | null;
  /** En puntos. */
  fontSize?: number | null;
  background?: string | null;
  foreground?: string | null;
  palette: string[];
  cursorShape?: "block" | "bar" | "underline" | null;
  cursorBlink?: boolean | null;
};

type Config = {
  fontFamily?: string;
  fontSize?: number;
  theme?: string;
  /** "auto" (por defecto), "off" o el nombre de una terminal. */
  terminal?: string;
  /** Buscar actualizaciones al abrir (por defecto sí). */
  autoUpdate?: boolean;
  /** Último modelo, esfuerzo y modo usados en Claude; los heredan las sesiones nuevas. */
  claude?: { model?: string; effort?: string; mode?: string };
};

// Roles de los temas *-ansi de Claude Code → índice en la paleta de la terminal.
const ANSI_ROLES: Record<string, number> = {
  "--claude": 9,
  "--claude-shimmer": 11,
  "--success": 2,
  "--error": 1,
  "--warning": 3,
  "--suggestion": 12,
  "--permission": 12,
  "--plan": 6,
  "--auto-accept": 13,
  "--bash": 13,
  "--diff-add": 2,
  "--diff-del": 1,
};

const DERIVED = ["--bg", "--text", "--bg-raised", "--bg-user", "--bg-hover", "--scroll-thumb", "--border-faint"];

function luminance(hex: string): number {
  const n = parseInt(hex.slice(1), 16);
  const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
}

class Prefs {
  config = $state<Config>({});
  claudeTheme = $state<string | null>(null);
  terminal = $state<TerminalStyle | null>(null);
  private media = matchMedia("(prefers-color-scheme: light)");

  /** Tema efectivo: el forzado en la app, si no el de Claude Code, si no oscuro. */
  get theme(): string {
    const t = this.config.theme ?? this.claudeTheme ?? "dark";
    if (t === "auto" || !THEMES.includes(t as any)) return this.media.matches ? "light" : "dark";
    return t;
  }

  get fontSize(): number {
    if (this.config.fontSize) return this.config.fontSize;
    const pt = this.terminal?.fontSize;
    return pt ? Math.round(((pt * 4) / 3) * 10) / 10 : DEFAULT_FONT_SIZE;
  }

  get fontFamily(): string | null {
    return this.config.fontFamily ?? this.terminal?.fontFamily ?? null;
  }

  async load() {
    this.config = await invoke<Config>("app_config");
    this.claudeTheme = await invoke<string | null>("claude_theme");
    await this.loadTerminal();
    this.media.addEventListener("change", () => this.apply());
  }

  async loadTerminal() {
    const which = this.config.terminal ?? "auto";
    this.terminal = which === "off" ? null : await invoke<TerminalStyle | null>("terminal_style", { which });
    this.apply();
  }

  apply() {
    const root = document.documentElement;
    const theme = this.theme;
    root.dataset.theme = theme;

    const family = this.fontFamily;
    if (family) root.style.setProperty("--font", `"${family}", var(--font-fallback)`);
    else root.style.removeProperty("--font");
    root.style.setProperty("--font-size", `${this.fontSize}px`);

    // Igual que Claude Code: el fondo y el texto son los de la terminal.
    for (const v of [...DERIVED, ...Object.keys(ANSI_ROLES)]) root.style.removeProperty(v);
    const t = this.terminal;
    if (t?.background && t.foreground) {
      root.style.setProperty("--bg", t.background);
      root.style.setProperty("--text", t.foreground);
      root.style.setProperty("--bg-raised", "color-mix(in srgb, var(--text) 5%, var(--bg))");
      root.style.setProperty("--bg-user", "color-mix(in srgb, var(--text) 10%, var(--bg))");
      root.style.setProperty("--bg-hover", "color-mix(in srgb, var(--text) 13%, var(--bg))");
      root.style.setProperty("--scroll-thumb", "color-mix(in srgb, var(--text) 20%, var(--bg))");
      root.style.setProperty("--border-faint", "color-mix(in srgb, var(--text) 18%, var(--bg))");
      root.style.colorScheme = luminance(t.background) > 0.5 ? "light" : "dark";
    } else {
      root.style.removeProperty("color-scheme");
    }

    const palette = t?.palette?.length === 16 ? t.palette : null;
    setPalette(palette);
    if (palette && theme.endsWith("-ansi")) {
      for (const [v, i] of Object.entries(ANSI_ROLES)) root.style.setProperty(v, palette[i]);
    }
  }

  async set(patch: Partial<Config>) {
    const next: Config = { ...this.config, ...patch };
    for (const k of Object.keys(next) as (keyof Config)[]) if (next[k] == null) delete next[k];
    const terminalChanged = next.terminal !== this.config.terminal;
    this.config = next;
    if (terminalChanged) await this.loadTerminal();
    else this.apply();
    await invoke("save_app_config", { config: next });
  }

  /** Última configuración de Claude, leída del disco por si la cambió otra ventana. */
  async lastClaude(): Promise<NonNullable<Config["claude"]>> {
    const c = await invoke<Config>("app_config").catch(() => this.config);
    return c.claude ?? {};
  }

  /** Recuerda la última configuración de Claude, como la TUI. */
  async rememberClaude(patch: NonNullable<Config["claude"]>) {
    await this.set({ claude: { ...(await this.lastClaude()), ...patch } });
  }

  zoom(delta: number) {
    if (delta === 0) return this.set({ fontSize: undefined });
    const size = Math.min(28, Math.max(9, Math.round(this.fontSize) + delta));
    return this.set({ fontSize: size });
  }
}

export const prefs = new Prefs();
