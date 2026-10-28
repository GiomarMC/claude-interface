// Convierte la salida ANSI de un statusline a HTML seguro.

import { escapeHtml } from "./markdown";

const XTERM16 = [
  "#000000", "#cd3131", "#0dbc79", "#e5e510", "#2472c8", "#bc3fbc", "#11a8cd", "#e5e5e5",
  "#666666", "#f14c4c", "#23d18b", "#f5f543", "#3b8eea", "#d670d6", "#29b8db", "#ffffff",
];

/** Los 16 colores básicos salen de la paleta de la terminal del usuario. */
let BASE16 = XTERM16;

export function setPalette(palette: string[] | null) {
  BASE16 = palette?.length === 16 ? palette : XTERM16;
}

function color256(n: number): string {
  if (n < 16) return BASE16[n];
  if (n < 232) {
    const i = n - 16;
    const steps = [0, 95, 135, 175, 215, 255];
    const r = steps[Math.floor(i / 36)];
    const g = steps[Math.floor(i / 6) % 6];
    const b = steps[i % 6];
    return `rgb(${r},${g},${b})`;
  }
  const v = 8 + (n - 232) * 10;
  return `rgb(${v},${v},${v})`;
}

type Style = {
  fg?: string;
  bg?: string;
  bold?: boolean;
  dim?: boolean;
  italic?: boolean;
  underline?: boolean;
  strike?: boolean;
  inverse?: boolean;
};

function applySgr(style: Style, params: number[]): Style {
  const s = { ...style };
  if (!params.length) params = [0];
  for (let i = 0; i < params.length; i++) {
    const p = params[i];
    if (p === 0) Object.keys(s).forEach((k) => delete (s as any)[k]);
    else if (p === 1) s.bold = true;
    else if (p === 2) s.dim = true;
    else if (p === 3) s.italic = true;
    else if (p === 4) s.underline = true;
    else if (p === 7) s.inverse = true;
    else if (p === 9) s.strike = true;
    else if (p === 22) s.bold = s.dim = false;
    else if (p === 23) s.italic = false;
    else if (p === 24) s.underline = false;
    else if (p === 27) s.inverse = false;
    else if (p === 29) s.strike = false;
    else if (p >= 30 && p <= 37) s.fg = BASE16[p - 30];
    else if (p >= 90 && p <= 97) s.fg = BASE16[p - 90 + 8];
    else if (p === 39) delete s.fg;
    else if (p >= 40 && p <= 47) s.bg = BASE16[p - 40];
    else if (p >= 100 && p <= 107) s.bg = BASE16[p - 100 + 8];
    else if (p === 49) delete s.bg;
    else if (p === 38 || p === 48) {
      const key = p === 38 ? "fg" : "bg";
      if (params[i + 1] === 5) {
        s[key] = color256(params[i + 2] ?? 0);
        i += 2;
      } else if (params[i + 1] === 2) {
        s[key] = `rgb(${params[i + 2] ?? 0},${params[i + 3] ?? 0},${params[i + 4] ?? 0})`;
        i += 4;
      }
    }
  }
  return s;
}

function css(s: Style): string {
  const fg = s.inverse ? (s.bg ?? "var(--bg)") : s.fg;
  const bg = s.inverse ? (s.fg ?? "var(--text)") : s.bg;
  const out: string[] = [];
  if (fg) out.push(`color:${fg}`);
  if (bg) out.push(`background:${bg}`);
  if (s.bold) out.push("font-weight:700");
  if (s.dim) out.push("opacity:.6");
  if (s.italic) out.push("font-style:italic");
  const deco = [s.underline && "underline", s.strike && "line-through"].filter(Boolean);
  if (deco.length) out.push(`text-decoration:${deco.join(" ")}`);
  return out.join(";");
}

export function ansiToHtml(input: string): string {
  let html = "";
  let style: Style = {};
  let link: string | null = null;
  // CSI ... m  |  OSC 8 ;; url (BEL | ST)  |  otras secuencias que descartamos
  const re = /\x1b\[([0-9;]*)([A-Za-z])|\x1b\]8;[^;]*;([^\x07\x1b]*)(?:\x07|\x1b\\)|\x1b\][^\x07]*(?:\x07|\x1b\\)/g;
  let last = 0;
  let m: RegExpExecArray | null;

  const emit = (text: string) => {
    if (!text) return;
    const c = css(style);
    let chunk = escapeHtml(text);
    if (c) chunk = `<span style="${c}">${chunk}</span>`;
    if (link && /^https?:\/\//.test(link)) chunk = `<a href="${escapeHtml(link)}">${chunk}</a>`;
    html += chunk;
  };

  while ((m = re.exec(input))) {
    emit(input.slice(last, m.index));
    last = re.lastIndex;
    if (m[2] === "m") {
      style = applySgr(style, m[1].split(";").filter((x) => x !== "").map(Number));
    } else if (m[3] !== undefined) {
      link = m[3] || null;
    }
  }
  emit(input.slice(last));
  return html;
}
