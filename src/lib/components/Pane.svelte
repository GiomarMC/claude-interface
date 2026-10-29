<script lang="ts">
  // Un panel: una terminal con el shell del usuario. Al escribir `claude`
  // encima aparece la interfaz de la app; al salir, se vuelve a la terminal
  // en el directorio en que quedó Claude.
  import { onDestroy, onMount, tick } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { ws, type Launch, type Pane } from "$lib/workspace.svelte";
  import Terminal from "./Terminal.svelte";
  import Chat from "./Chat.svelte";

  let { pane, active }: { pane: Pane; active: boolean } = $props();

  let term = $state<ReturnType<typeof Terminal>>();
  let chat = $state<ReturnType<typeof Chat>>();

  // Opciones de `claude` que la interfaz entiende; con cualquier otra cosa
  // (subcomandos, -p, --model…) se abre el Claude Code de siempre.
  const SUBCOMMANDS = new Set(["mcp", "config", "update", "doctor", "install", "setup-token", "migrate-installer", "plugin", "plugins", "auth", "agents", "help"]);

  function parseArgs(args: string[]): Launch | null {
    const l: Launch = {};
    for (let i = 0; i < args.length; i++) {
      const a = args[i];
      if (a === "-c" || a === "--continue") l.continue = true;
      else if (a === "-r" || a === "--resume") {
        const next = args[i + 1];
        if (next && !next.startsWith("-")) {
          l.resume = next;
          i++;
        } else l.picker = true;
      } else if (a.startsWith("--resume=")) l.resume = a.slice(9);
      else if (!a.startsWith("-") && !l.prompt && !(i === 0 && SUBCOMMANDS.has(a))) l.prompt = a;
      else return null;
    }
    return l;
  }

  function onClaude(cwd: string, args: string[]) {
    const launch = parseArgs(args);
    // Respuesta inmediata: la función `claude` del shell la espera unos segundos.
    term?.send(launch ? "ok\r" : "run\r");
    if (!launch) return;
    pane.cwd = cwd;
    pane.launch = launch;
    pane.waiting = true;
    pane.mode = "claude";
  }

  /** Cierra la interfaz de Claude y devuelve el control al shell. */
  function exitClaude() {
    const cwd = pane.session.cwd || pane.cwd;
    pane.mode = "shell";
    pane.launch = null;
    if (pane.waiting) term?.send(`cd:${cwd}\r`);
    pane.waiting = false;
    tick().then(() => active && term?.focus());
  }

  function onShellExit() {
    if (!ws.closePane(pane.id)) getCurrentWindow().close();
  }

  /** Rutas entre comillas para el shell, como al arrastrar a Konsole. */
  function quote(p: string): string {
    if (navigator.userAgent.includes("Windows")) return `"${p}"`;
    return /^[\w@%+=:,./-]+$/.test(p) ? p : `'${p.replace(/'/g, `'\\''`)}'`;
  }

  function focus() {
    if (pane.mode === "claude") chat?.focus();
    else term?.focus();
  }

  onMount(() => {
    ws.api.set(pane.id, {
      focus,
      addPaths: (paths) => (pane.mode === "claude" ? chat?.addPaths(paths) : term?.paste(paths.map(quote).join(" ") + " ")),
      pickFiles: () => chat?.pickFiles(),
      exportMd: () => chat?.exportMd(),
      copy: () => {
        const text = pane.mode === "shell" ? term?.copySelection() : null;
        if (!text) return false;
        writeText(text);
        return true;
      },
      paste: (text) => term?.paste(text),
      ctrlC: () => (pane.mode === "claude" ? (chat?.ctrlC() ?? false) : false),
    });
    pane.session.init();
  });

  onDestroy(() => {
    ws.api.delete(pane.id);
    pane.session.dispose();
  });

  // Al activarse por teclado (o al abrirse), el foco pasa a este panel. Con el
  // ratón se deja que el click lo ponga donde corresponde.
  let byPointer = false;
  $effect(() => {
    void pane.mode;
    if (!active) return;
    if (byPointer) byPointer = false;
    else tick().then(focus);
  });

  function onPointerDown() {
    if (!active) byPointer = true;
    ws.focusPane(pane.id);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="pane" onpointerdown={onPointerDown}>
  <div class="layer" class:hidden={pane.mode === "claude"}>
    <Terminal
      bind:this={term}
      key={pane.session.key}
      cwd={pane.cwd}
      onclaude={onClaude}
      oncwd={(cwd) => (pane.cwd = cwd)}
      ontitle={(t) => (pane.title = t)}
      onexit={onShellExit}
    />
  </div>
  {#if pane.mode === "claude"}
    <Chat bind:this={chat} {pane} {active} onexit={exitClaude} />
  {/if}
</div>

<style>
  .pane {
    position: absolute;
    inset: 0;
    background: var(--bg);
  }
  .layer {
    position: absolute;
    inset: 0;
  }
  .layer.hidden {
    visibility: hidden;
  }
</style>
