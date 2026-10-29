// Pestañas y paneles divididos de una ventana, como en Konsole o tmux.
// Cada panel tiene su propia sesión de `claude`. La disposición es un árbol
// de divisiones, pero los paneles se dibujan en una lista plana (ver
// `layout`) para que dividir o mover no vuelva a montar sus componentes.
import { Session } from "./session.svelte";

/** Cómo abrir la interfaz de Claude (de `claude <args>` en la terminal, o al retomar tras actualizar). */
export type Launch = { resume?: string; continue?: boolean; picker?: boolean; prompt?: string };

/**
 * Un panel es una terminal con el shell del usuario. Al escribir `claude`,
 * el panel muestra la interfaz de la app (`mode: "claude"`) hasta /exit.
 */
export type Pane = {
  kind: "pane";
  id: string;
  session: Session;
  /** Directorio del shell (OSC 7), o el inicial. */
  cwd: string;
  /** Título que pone el shell o el programa que corre en la terminal. */
  title: string;
  mode: "shell" | "claude";
  launch: Launch | null;
  /** La función `claude` del shell espera respuesta al salir de la interfaz. */
  waiting: boolean;
};
/** `row`: hijos lado a lado; `column`: uno sobre otro. */
export type Split = { kind: "split"; id: string; dir: "row" | "column"; children: LayoutNode[]; sizes: number[] };
export type LayoutNode = Pane | Split;
export type Tab = { id: string; root: LayoutNode; focus: string; zoomed: boolean };

/** Fracciones (0–1) del área de la pestaña. */
export type Rect = { x: number; y: number; w: number; h: number };
export type Divider = { split: string; index: number; dir: "row" | "column"; at: number; area: Rect };

export type PaneApi = {
  focus(): void;
  addPaths(paths: string[]): void;
  pickFiles(): void;
  exportMd(): void;
  /** Copia la selección de la terminal; false si no hay. */
  copy(): boolean;
  paste(text: string): void;
  /** Ctrl+C en la interfaz de Claude; false si no aplica (p. ej. copiar). */
  ctrlC(): boolean;
};

export type Direction = "left" | "right" | "up" | "down";

const MIN_SIZE = 0.12;

function panesOf(node: LayoutNode): Pane[] {
  return node.kind === "pane" ? [node] : node.children.flatMap(panesOf);
}

/** Nodo padre de `id` (null si es la raíz o no está). */
function parentOf(node: LayoutNode, id: string): Split | null {
  if (node.kind === "pane") return null;
  for (const c of node.children) {
    if (c.id === id) return node;
    const p = parentOf(c, id);
    if (p) return p;
  }
  return null;
}

function findNode(node: LayoutNode, id: string): LayoutNode | null {
  if (node.id === id) return node;
  if (node.kind === "pane") return null;
  for (const c of node.children) {
    const n = findNode(c, id);
    if (n) return n;
  }
  return null;
}

class Workspace {
  /** Etiqueta de la ventana de Tauri; prefijo de las claves de sesión. */
  label = "main";
  home = $state("");
  tabs = $state<Tab[]>([]);
  active = $state(0);
  /** Funciones de cada panel montado, para atajos y arrastrar archivos. */
  api = new Map<string, PaneApi>();
  private counter = 0;

  get tab(): Tab | undefined {
    return this.tabs[this.active];
  }

  get pane(): Pane | null {
    const t = this.tab;
    return t ? (panesOf(t.root).find((p) => p.id === t.focus) ?? null) : null;
  }

  /** Todos los paneles de todas las pestañas, con la pestaña a la que pertenecen. */
  get allPanes(): { pane: Pane; tab: Tab }[] {
    return this.tabs.flatMap((tab) => panesOf(tab.root).map((pane) => ({ pane, tab })));
  }

  panes(tab: Tab): Pane[] {
    return panesOf(tab.root);
  }

  private newPane(cwd: string, launch?: Launch): Pane {
    const id = String(++this.counter);
    return {
      kind: "pane",
      id,
      session: new Session(`${this.label}:${id}`),
      cwd,
      title: "",
      mode: launch ? "claude" : "shell",
      launch: launch ?? null,
      waiting: false,
    };
  }

  /** Directorio actual de un panel: el de Claude si está abierto, si no el del shell. */
  paneCwd(p: Pane | null): string {
    if (!p) return this.home;
    return (p.mode === "claude" && p.session.cwd) || p.cwd || this.home;
  }

  /** Directorio actual del panel enfocado, para abrir los nuevos ahí (como Konsole). */
  get cwd(): string {
    return this.paneCwd(this.pane);
  }

  newTab(cwd = this.cwd, launch?: Launch) {
    const p = this.newPane(cwd, launch);
    this.tabs.push({ id: p.id, root: p, focus: p.id, zoomed: false });
    this.active = this.tabs.length - 1;
  }

  split(dir: "row" | "column") {
    const tab = this.tab;
    const cur = this.pane;
    if (!tab || !cur) return;
    const p = this.newPane(this.cwd);
    const parent = parentOf(tab.root, cur.id);
    if (parent && parent.dir === dir) {
      const i = parent.children.findIndex((c) => c.id === cur.id);
      parent.children.splice(i + 1, 0, p);
      parent.sizes = parent.children.map(() => 1 / parent.children.length);
    } else {
      const node: Split = { kind: "split", id: `s${p.id}`, dir, children: [cur, p], sizes: [0.5, 0.5] };
      if (!parent) tab.root = node;
      else parent.children[parent.children.findIndex((c) => c.id === cur.id)] = node;
    }
    tab.focus = p.id;
    tab.zoomed = false;
  }

  /** Cierra un panel. Devuelve false si era el último de la ventana. */
  closePane(id = this.pane?.id): boolean {
    const ti = this.tabs.findIndex((t) => panesOf(t.root).some((p) => p.id === id));
    const tab = this.tabs[ti];
    if (!tab || !id) return true;
    const parent = parentOf(tab.root, id);
    if (!parent) return this.closeTab(ti);
    const i = parent.children.findIndex((c) => c.id === id);
    parent.children.splice(i, 1);
    parent.sizes.splice(i, 1);
    const total = parent.sizes.reduce((a, b) => a + b, 0);
    parent.sizes = parent.sizes.map((s) => s / total);
    // Una división con un solo hijo se reemplaza por ese hijo.
    if (parent.children.length === 1) {
      const only = parent.children[0];
      const grand = parentOf(tab.root, parent.id);
      if (!grand) tab.root = only;
      else grand.children[grand.children.findIndex((c) => c.id === parent.id)] = only;
    }
    if (tab.focus === id) {
      const sibling = parent.children[Math.max(0, i - 1)] ?? parent.children[0];
      tab.focus = panesOf(findNode(tab.root, sibling.id) ?? tab.root)[0].id;
    }
    tab.zoomed = false;
    return true;
  }

  closeTab(index = this.active): boolean {
    this.tabs.splice(index, 1);
    if (!this.tabs.length) return false;
    if (this.active >= index && this.active > 0) this.active--;
    this.active = Math.min(this.active, this.tabs.length - 1);
    return true;
  }

  gotoTab(index: number) {
    if (index >= 0 && index < this.tabs.length) this.active = index;
  }

  cycleTab(delta: number) {
    const n = this.tabs.length;
    this.active = (this.active + delta + n) % n;
  }

  moveTab(delta: number) {
    const to = this.active + delta;
    if (to < 0 || to >= this.tabs.length) return;
    const [t] = this.tabs.splice(this.active, 1);
    this.tabs.splice(to, 0, t);
    this.active = to;
  }

  focusPane(id: string) {
    const ti = this.tabs.findIndex((t) => panesOf(t.root).some((p) => p.id === id));
    if (ti < 0) return;
    this.active = ti;
    this.tabs[ti].focus = id;
  }

  cyclePane(delta: number) {
    const tab = this.tab;
    if (!tab) return;
    const list = panesOf(tab.root);
    const i = list.findIndex((p) => p.id === tab.focus);
    tab.focus = list[(i + delta + list.length) % list.length].id;
    tab.zoomed = false;
  }

  /** Enfoca el panel vecino en esa dirección, como focus-view-left de Konsole. */
  focusDirection(dir: Direction) {
    const tab = this.tab;
    if (!tab) return;
    const { panes } = this.layout(tab, false);
    const cur = panes.find((p) => p.id === tab.focus);
    if (!cur) return;
    const c = cur.rect;
    const eps = 1e-6;
    const overlap = (a0: number, a1: number, b0: number, b1: number) => Math.min(a1, b1) - Math.max(a0, b0);
    let best: { id: string; dist: number; ov: number } | null = null;
    for (const { id, rect: r } of panes) {
      if (id === cur.id) continue;
      const horizontal = dir === "left" || dir === "right";
      const dist =
        dir === "left" ? c.x - (r.x + r.w) : dir === "right" ? r.x - (c.x + c.w) : dir === "up" ? c.y - (r.y + r.h) : r.y - (c.y + c.h);
      const ov = horizontal ? overlap(c.y, c.y + c.h, r.y, r.y + r.h) : overlap(c.x, c.x + c.w, r.x, r.x + r.w);
      if (dist < -eps || ov <= eps) continue;
      if (!best || dist < best.dist - eps || (Math.abs(dist - best.dist) <= eps && ov > best.ov)) best = { id, dist, ov };
    }
    if (best) {
      tab.focus = best.id;
      tab.zoomed = false;
    }
  }

  /** Posición de cada panel y de cada separador. Con `zoomed`, solo el enfocado. */
  layout(tab: Tab, honorZoom = true): { panes: { id: string; rect: Rect }[]; dividers: Divider[] } {
    const out = { panes: [] as { id: string; rect: Rect }[], dividers: [] as Divider[] };
    if (honorZoom && tab.zoomed) {
      out.panes.push({ id: tab.focus, rect: { x: 0, y: 0, w: 1, h: 1 } });
      return out;
    }
    const walk = (node: LayoutNode, r: Rect) => {
      if (node.kind === "pane") return void out.panes.push({ id: node.id, rect: r });
      const total = node.sizes.reduce((a, b) => a + b, 0) || 1;
      let off = 0;
      node.children.forEach((child, i) => {
        const f = node.sizes[i] / total;
        const cr =
          node.dir === "row"
            ? { x: r.x + off * r.w, y: r.y, w: f * r.w, h: r.h }
            : { x: r.x, y: r.y + off * r.h, w: r.w, h: f * r.h };
        if (i > 0) out.dividers.push({ split: node.id, index: i - 1, dir: node.dir, at: node.dir === "row" ? cr.x : cr.y, area: r });
        walk(child, cr);
        off += f;
      });
    };
    walk(tab.root, { x: 0, y: 0, w: 1, h: 1 });
    return out;
  }

  /** Mueve el separador `index` de una división a `pos` (fracción de la pestaña). */
  resize(tab: Tab, splitId: string, index: number, pos: number) {
    const node = findNode(tab.root, splitId);
    const d = this.layout(tab, false).dividers.find((x) => x.split === splitId && x.index === index);
    if (!node || node.kind !== "split" || !d) return;
    const start = node.dir === "row" ? d.area.x : d.area.y;
    const len = node.dir === "row" ? d.area.w : d.area.h;
    const total = node.sizes.reduce((a, b) => a + b, 0);
    const before = node.sizes.slice(0, index).reduce((a, b) => a + b, 0) / total;
    const pair = (node.sizes[index] + node.sizes[index + 1]) / total;
    const frac = Math.min(before + pair - MIN_SIZE, Math.max(before + MIN_SIZE, (pos - start) / len));
    if (pair < 2 * MIN_SIZE) return;
    node.sizes[index] = (frac - before) * total;
    node.sizes[index + 1] = (before + pair - frac) * total;
  }
}

export const ws = new Workspace();
