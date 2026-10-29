<script lang="ts">
  // Terminal real (xterm.js) conectada al pseudo-terminal del panel en el
  // backend (pty.rs). Escucha dos secuencias de la integración del shell:
  // OSC 633 "ClaudeInterface" (el usuario escribió `claude`) y OSC 7 (cwd).
  import "@xterm/xterm/css/xterm.css";
  import { onDestroy, onMount } from "svelte";
  import { Channel, invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import { Unicode11Addon } from "@xterm/addon-unicode11";
  import { prefs } from "$lib/prefs.svelte";

  let {
    key,
    cwd,
    onclaude,
    oncwd,
    ontitle,
    onexit,
  }: {
    key: string;
    cwd: string;
    onclaude: (cwd: string, args: string[]) => void;
    oncwd: (cwd: string) => void;
    ontitle: (title: string) => void;
    onexit: () => void;
  } = $props();

  let el: HTMLDivElement;
  let term: Terminal;
  let fit: FitAddon;
  let observer: ResizeObserver | undefined;
  let alive = false;

  const decoder = new TextDecoder();
  const b64 = (s: string) => decoder.decode(Uint8Array.from(atob(s), (c) => c.charCodeAt(0)));

  function cssVar(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  /** Colores y fuente: los de la terminal que se imita, o los del tema. */
  function applyStyle() {
    if (!term) return;
    const t = prefs.terminal;
    const palette = t?.palette?.length === 16 ? t.palette : null;
    const bg = cssVar("--bg");
    const fg = cssVar("--text");
    const names = [
      "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
      "brightBlack", "brightRed", "brightGreen", "brightYellow", "brightBlue", "brightMagenta", "brightCyan", "brightWhite",
    ] as const;
    const theme: Record<string, string> = {
      background: bg,
      foreground: fg,
      cursor: fg,
      cursorAccent: bg,
      selectionBackground: /^#[0-9a-f]{6}$/i.test(fg) ? `${fg}4d` : "rgba(128, 128, 128, 0.3)",
    };
    if (palette) names.forEach((n, i) => (theme[n] = palette[i]));
    term.options.theme = theme;
    term.options.fontFamily = cssVar("--font") || "monospace";
    term.options.fontSize = prefs.fontSize;
    term.options.cursorStyle = t?.cursorShape ?? "block";
    term.options.cursorBlink = t?.cursorBlink ?? true;
    refit();
  }

  function refit() {
    if (!term || !el?.offsetWidth || !el.offsetHeight) return;
    try {
      fit.fit();
    } catch {}
  }

  onMount(() => {
    term = new Terminal({
      allowProposedApi: true,
      cursorBlink: true,
      scrollback: 10000,
      macOptionIsMeta: true,
      drawBoldTextInBrightColors: false,
    });
    fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon((_e, uri) => openUrl(uri)));
    term.loadAddon(new Unicode11Addon());
    term.unicode.activeVersion = "11";
    term.open(el);
    applyStyle();

    term.parser.registerOscHandler(633, (data) => {
      const m = data.match(/^ClaudeInterface;([^;]*);?(.*)$/);
      if (!m) return false;
      onclaude(b64(m[1]), m[2] ? b64(m[2]).split("\0").filter((a) => a !== "") : []);
      return true;
    });
    term.parser.registerOscHandler(7, (data) => {
      const m = data.match(/^file:\/\/[^/]*(\/.*)$/);
      if (!m) return false;
      let path = m[1];
      try {
        path = decodeURIComponent(path);
      } catch {}
      // Windows: file://host/C:/…
      if (/^\/[A-Za-z]:/.test(path)) path = path.slice(1);
      oncwd(path);
      return true;
    });
    term.onTitleChange((t) => ontitle(t));

    // Shift+RePág/AvPág recorren el historial, como en Konsole.
    term.attachCustomKeyEventHandler((e) => {
      if (e.type === "keydown" && e.shiftKey && !e.ctrlKey && !e.altKey && (e.key === "PageUp" || e.key === "PageDown")) {
        term.scrollPages(e.key === "PageUp" ? -1 : 1);
        return false;
      }
      return true;
    });

    term.onData((data) => alive && invoke("pty_write", { key, data }).catch(() => {}));
    term.onBinary((data) => alive && invoke("pty_write", { key, data }).catch(() => {}));
    term.onResize(({ cols, rows }) => alive && invoke("pty_resize", { key, cols, rows }));

    const channel = new Channel<{ kind: "data"; data: string } | { kind: "exit" }>();
    channel.onmessage = (ev) => {
      if (ev.kind === "data") term.write(ev.data);
      else {
        alive = false;
        onexit();
      }
    };
    invoke("pty_spawn", { key, cwd, cols: term.cols, rows: term.rows, onEvent: channel })
      .then(() => {
        alive = true;
        refit();
      })
      .catch((e) => term.write(`\r\n\x1b[31mNo se pudo abrir la terminal: ${e}\x1b[0m\r\n`));

    observer = new ResizeObserver(() => refit());
    observer.observe(el);
  });

  onDestroy(() => {
    observer?.disconnect();
    alive = false;
    invoke("pty_kill", { key }).catch(() => {});
    term?.dispose();
  });

  $effect(() => {
    void [prefs.terminal, prefs.fontSize, prefs.fontFamily, prefs.theme];
    // Las variables CSS se actualizan en prefs.apply(); se leen después.
    requestAnimationFrame(applyStyle);
  });

  export function focus() {
    term?.focus();
    refit();
  }

  /** Escribe en el shell como si se tecleara. */
  export function send(data: string) {
    if (alive) invoke("pty_write", { key, data }).catch(() => {});
  }

  export function paste(text: string) {
    term?.paste(text);
  }

  export function copySelection(): string | null {
    return term?.hasSelection() ? term.getSelection() : null;
  }
</script>

<div class="term" bind:this={el}></div>

<style>
  .term {
    position: absolute;
    inset: 0.3em 0 0 1ch;
  }
  .term :global(.xterm) {
    height: 100%;
  }
  .term :global(.xterm-viewport) {
    background: transparent !important;
  }
</style>
