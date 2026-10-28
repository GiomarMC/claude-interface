// Mismos formatos que statusline-command.sh

export function fmtNum(n?: number): string {
  if (n == null) return "";
  if (n >= 1_000_000) {
    const m = Math.floor(n / 1_000_000);
    const r = Math.floor((n % 1_000_000) / 100_000);
    return r === 0 ? `${m}M` : `${m}.${r}M`;
  }
  if (n >= 1000) return `${Math.floor(n / 1000)}k`;
  return `${n}`;
}

export function fmtMs(ms?: number): string {
  if (ms == null) return "";
  const s = Math.floor(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sc = s % 60;
  if (h > 0) return `${h}h ${m}m ${sc}s`;
  if (m > 0) return `${m}m ${sc}s`;
  return `${sc}s`;
}

export function fmtReset(epoch?: number): string {
  if (!epoch) return "";
  const diff = epoch - Math.floor(Date.now() / 1000);
  if (diff <= 0) return "now";
  const d = Math.floor(diff / 86400);
  const h = Math.floor((diff % 86400) / 3600);
  const m = Math.floor((diff % 3600) / 60);
  return d > 0 ? `${d}d ${h}h` : `${h}h ${m}m`;
}

export function fmtCost(usd: number): string {
  return usd.toFixed(4).replace(/0*$/, "").replace(/\.$/, "");
}

export function fmtBytes(n: number): string {
  if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  if (n >= 1024) return `${Math.round(n / 1024)} KB`;
  return `${n} B`;
}

export function barColor(pct: number): string {
  if (pct < 50) return "var(--sl-green)";
  if (pct < 70) return "var(--sl-cyan)";
  if (pct < 85) return "var(--sl-amber)";
  return "var(--sl-red)";
}

/** "claude-opus-5-5" → "Opus 5.5" */
export function modelName(id?: string): string {
  if (!id) return "";
  const m = id.match(/claude-([a-z]+)-(\d+)(?:-(\d+))?/);
  if (!m) return id;
  const fam = m[1][0].toUpperCase() + m[1].slice(1);
  return m[3] && m[3].length < 3 ? `${fam} ${m[2]}.${m[3]}` : `${fam} ${m[2]}`;
}

export function timeAgo(epochSec: number): string {
  const diff = Date.now() / 1000 - epochSec;
  if (diff < 60) return "ahora";
  if (diff < 3600) return `hace ${Math.floor(diff / 60)} min`;
  if (diff < 86400) return `hace ${Math.floor(diff / 3600)} h`;
  return `hace ${Math.floor(diff / 86400)} d`;
}

export function shortPath(p: string, home?: string): string {
  return home && p.startsWith(home) ? "~" + p.slice(home.length) : p;
}
