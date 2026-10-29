<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { homeDir } from "@tauri-apps/api/path";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { prefs } from "$lib/prefs.svelte";
  import { updater, RESUME_KEY } from "$lib/updater.svelte";
  import { keymap, type Action } from "$lib/keys.svelte";
  import { ws, type Tab } from "$lib/workspace.svelte";
  import Setup, { type ClaudeStatus } from "$lib/components/Setup.svelte";
  import Pane from "$lib/components/Pane.svelte";

  let dragging = $state(false);
  let area = $state<HTMLDivElement>();
  /** Estado de Claude Code si hace falta el asistente de primer arranque. */
  let setup = $state<ClaudeStatus | null>(null);
  let setupDone: (() => void) | undefined;

  const CWD_KEY = "ci.cwd";
  const win = getCurrentWindow();
  ws.label = win.label;
  const isMain = win.label === "main";

  onMount(() => {
    let unlistenDrop: (() => void) | undefined;
    (async () => {
      await prefs.load();
      ws.home = (await homeDir()).replace(/\/$/, "");

      // Sin Claude Code instalado o sin sesión iniciada, primero el asistente.
      const status = await invoke<ClaudeStatus>("claude_status");
      if (!status.installed || status.loggedIn === false) {
        setup = status;
        await new Promise<void>((resolve) => (setupDone = resolve));
      }
      // Ventanas abiertas con "nueva ventana": empiezan en el directorio del panel de origen.
      const fromWindow = new URLSearchParams(location.search).get("cwd");
      let last: string | null = null;
      try {
        last = localStorage.getItem(CWD_KEY);
      } catch {}
      // Tras instalar una actualización se retoma la conversación que estaba abierta.
      let resume: { sessionId: string; cwd: string } | null = null;
      if (isMain) {
        try {
          resume = JSON.parse(localStorage.getItem(RESUME_KEY) ?? "null");
          localStorage.removeItem(RESUME_KEY);
        } catch {}
      }
      if (resume) ws.newTab(resume.cwd, resume.sessionId);
      else ws.newTab(fromWindow ?? (await invoke<string | null>("initial_cwd")) ?? last ?? ws.home);
      if (isMain) updater.start(prefs.config.autoUpdate !== false);

      unlistenDrop = await getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "enter" || p.type === "over") dragging = true;
        else if (p.type === "leave") dragging = false;
        else if (p.type === "drop") {
          dragging = false;
          const id = ws.pane?.id;
          if (id) ws.api.get(id)?.addPaths(p.paths);
        }
      });
    })();
    return () => {
      unlistenDrop?.();
      if (isMain) updater.stop();
    };
  });

  // Los atajos son los de la terminal que se imita (/terminal).
  $effect(() => {
    keymap.load(prefs.config.terminal ?? "auto");
  });

  const dirName = (cwd: string) => cwd.split(/[\\/]/).filter(Boolean).pop() || cwd;

  $effect(() => {
    const cwd = ws.pane?.session.cwd;
    if (cwd) win.setTitle(`Claude · ${dirName(cwd)}`);
  });

  function newWindow() {
    const bg = getComputedStyle(document.documentElement).getPropertyValue("--bg").trim();
    new WebviewWindow(`w${Date.now()}`, {
      url: `/?cwd=${encodeURIComponent(ws.cwd)}`,
      title: "Claude Interface",
      width: 1180,
      height: 860,
      minWidth: 560,
      minHeight: 420,
      dragDropEnabled: true,
      ...(bg.startsWith("#") ? { backgroundColor: bg } : {}),
    });
  }

  function closePane() {
    if (!ws.closePane()) win.close();
  }

  function closeTab(i = ws.active) {
    if (!ws.closeTab(i)) win.close();
  }

  /** Ejecuta la acción de un atajo. Devuelve false si no aplica y la tecla debe seguir su camino. */
  function run(action: Action): boolean {
    const tab = ws.tab;
    const multiPane = !!tab && ws.panes(tab).length > 1;
    const multiTab = ws.tabs.length > 1;
    const pane = ws.pane;
    const api = pane ? ws.api.get(pane.id) : undefined;
    const goto = action.match(/^goto_tab_(\d+)$/);
    if (goto) {
      const i = Number(goto[1]) - 1;
      if (i >= ws.tabs.length) return false;
      ws.gotoTab(i);
      return true;
    }
    switch (action) {
      case "new_tab":
        ws.newTab();
        return true;
      case "new_window":
        newWindow();
        return true;
      case "close_pane":
        closePane();
        return true;
      case "close_tab":
        closeTab();
        return true;
      case "split_right":
        ws.split("row");
        return true;
      case "split_down":
        ws.split("column");
        return true;
      // Como en las terminales, moverse entre pestañas o paneles solo
      // captura la tecla cuando hay a dónde moverse.
      case "next_tab":
      case "prev_tab":
        if (!multiTab) return false;
        ws.cycleTab(action === "next_tab" ? 1 : -1);
        return true;
      case "move_tab_left":
      case "move_tab_right":
        if (!multiTab) return false;
        ws.moveTab(action === "move_tab_left" ? -1 : 1);
        return true;
      case "focus_left":
      case "focus_right":
      case "focus_up":
      case "focus_down":
        if (!multiPane) return false;
        ws.focusDirection(action.slice(6) as "left");
        return true;
      case "next_pane":
      case "prev_pane":
        if (!multiPane) return false;
        ws.cyclePane(action === "next_pane" ? 1 : -1);
        return true;
      case "maximize_pane":
        if (!tab || !multiPane) return false;
        tab.zoomed = !tab.zoomed;
        return true;
      case "zoom_in":
        prefs.zoom(1);
        return true;
      case "zoom_out":
        prefs.zoom(-1);
        return true;
      case "zoom_reset":
        prefs.zoom(0);
        return true;
      case "export":
        api?.exportMd();
        return true;
      case "copy": {
        const text = window.getSelection()?.toString();
        if (!text) return false;
        writeText(text);
        return true;
      }
    }
    return false;
  }

  // Los atajos de la terminal van primero, como en un emulador: la app de
  // dentro (aquí, el cuadro de texto) solo recibe las teclas que no usa.
  function onKeyCapture(e: KeyboardEvent) {
    if (setup || e.isComposing || !ws.tab) return;
    const action = keymap.match(e);
    if (action && run(action)) {
      e.preventDefault();
      e.stopPropagation();
    }
  }

  function onKey(e: KeyboardEvent) {
    const s = ws.pane?.session;
    if (!s) return;
    if (e.key === "Escape" && s.busy && !s.permission) s.interrupt();
    else if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "o") {
      e.preventDefault();
      ws.api.get(ws.pane!.id)?.pickFiles();
    }
  }

  // ── Separadores ──────────────────────────────────────────────────

  function startResize(e: PointerEvent, tab: Tab, split: string, index: number, dir: "row" | "column") {
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      const r = area!.getBoundingClientRect();
      const pos = dir === "row" ? (ev.clientX - r.left) / r.width : (ev.clientY - r.top) / r.height;
      ws.resize(tab, split, index, pos);
    };
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
  }

  const layouts = $derived(new Map(ws.tabs.map((t) => [t.id, ws.layout(t)])));
  const pct = (n: number) => `${n * 100}%`;
</script>

<svelte:window onkeydowncapture={onKeyCapture} onkeydown={onKey} />

<main>
  {#if setup}
    <div class="scroll">
      <Setup
        status={setup}
        onready={() => {
          setup = null;
          setupDone?.();
        }}
      />
    </div>
  {:else}
    {#if ws.tabs.length > 1 || ws.tab?.zoomed}
      <nav class="tabs" aria-label="Pestañas">
        {#each ws.tabs as tab, i (tab.id)}
          {@const panes = ws.panes(tab)}
          {@const focused = panes.find((p) => p.id === tab.focus) ?? panes[0]}
          {@const waiting = panes.some((p) => p.session.permission)}
          {@const busy = panes.some((p) => p.session.busy)}
          <button
            class="tab"
            class:active={i === ws.active}
            onclick={() => ws.gotoTab(i)}
            onauxclick={(e) => e.button === 1 && closeTab(i)}
            title={focused.session.cwd}
          >
            <span class="num">{i + 1}</span>
            <span class="name">{dirName(focused.session.cwd || focused.cwd)}</span>
            {#if panes.length > 1}<span class="faint">[{panes.length}]</span>{/if}
            {#if tab.zoomed}<span class="faint">[Z]</span>{/if}
            {#if waiting}<span class="warn">!</span>{:else if busy}<span class="claude">✻</span>{/if}
          </button>
        {/each}
      </nav>
    {/if}

    <div class="area" bind:this={area}>
      {#each ws.allPanes as { pane, tab } (pane.id)}
        {@const rect = layouts.get(tab.id)?.panes.find((p) => p.id === pane.id)?.rect}
        {@const visible = tab === ws.tab && !!rect}
        <div
          class="slot"
          class:hidden={!visible}
          class:focused={visible && tab.focus === pane.id && ws.panes(tab).length > 1 && !tab.zoomed}
          style:left={pct(rect?.x ?? 0)}
          style:top={pct(rect?.y ?? 0)}
          style:width={pct(rect?.w ?? 1)}
          style:height={pct(rect?.h ?? 1)}
        >
          <Pane {pane} active={visible && tab.focus === pane.id} />
        </div>
      {/each}

      {#if ws.tab}
        {@const tab = ws.tab}
        {#each layouts.get(tab.id)?.dividers ?? [] as d (d.split + d.index)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="divider {d.dir}"
            style:left={d.dir === "row" ? pct(d.at) : pct(d.area.x)}
            style:top={d.dir === "row" ? pct(d.area.y) : pct(d.at)}
            style:width={d.dir === "row" ? null : pct(d.area.w)}
            style:height={d.dir === "row" ? pct(d.area.h) : null}
            onpointerdown={(e) => startResize(e, tab, d.split, d.index, d.dir)}
          ></div>
        {/each}
      {/if}
    </div>
  {/if}

  {#if dragging}
    <div class="drop">
      <div class="drop-box">
        <div class="claude bold">✻ Suelta para adjuntar</div>
        <div class="dim">texto → se incluye el contenido · imágenes/PDF → se envían directo · carpetas → por ruta</div>
      </div>
    </div>
  {/if}
</main>

<style>
  main {
    height: 100vh;
    display: flex;
    flex-direction: column;
    position: relative;
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding-top: 1em;
  }
  .claude {
    color: var(--claude);
  }
  .warn {
    color: var(--warning);
  }
  .tabs {
    flex: none;
    display: flex;
    overflow-x: auto;
    border-bottom: 1px solid var(--border-faint);
    scrollbar-width: none;
  }
  .tab {
    display: flex;
    gap: 0.8ch;
    align-items: baseline;
    padding: 0.25em 1.5ch;
    color: var(--text-dim);
    white-space: nowrap;
    border-right: 1px solid var(--border-faint);
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
  }
  .tab:hover {
    color: var(--text);
    background: var(--bg-hover);
  }
  .tab.active {
    color: var(--text);
    border-bottom-color: var(--claude);
  }
  .tab.active .num {
    color: var(--claude);
  }
  .num {
    color: var(--text-faint);
  }
  .name {
    max-width: 24ch;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .area {
    flex: 1;
    position: relative;
    min-height: 0;
  }
  .slot {
    position: absolute;
    overflow: hidden;
  }
  /* Ocultos sin display:none, para que conserven el scroll. */
  .slot.hidden {
    visibility: hidden;
    pointer-events: none;
    z-index: -1;
  }
  .slot.focused::after {
    content: "";
    position: absolute;
    inset: 0;
    border: 1px solid color-mix(in srgb, var(--claude) 55%, transparent);
    pointer-events: none;
  }
  .divider {
    position: absolute;
    z-index: 2;
  }
  .divider::before {
    content: "";
    position: absolute;
    background: var(--border-faint);
  }
  .divider.row {
    width: 7px;
    margin-left: -3px;
    cursor: col-resize;
  }
  .divider.row::before {
    left: 3px;
    width: 1px;
    top: 0;
    bottom: 0;
  }
  .divider.column {
    height: 7px;
    margin-top: -3px;
    cursor: row-resize;
  }
  .divider.column::before {
    top: 3px;
    height: 1px;
    left: 0;
    right: 0;
  }
  .divider:hover::before {
    background: var(--claude);
  }
  .drop {
    position: absolute;
    inset: 0;
    background: color-mix(in srgb, var(--bg) 85%, transparent);
    display: grid;
    place-items: center;
    pointer-events: none;
    z-index: 5;
  }
  .drop-box {
    border: 1px dashed var(--claude);
    border-radius: 10px;
    padding: 2em 4ch;
    text-align: center;
  }
</style>
