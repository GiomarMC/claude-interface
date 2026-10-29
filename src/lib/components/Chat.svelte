<script lang="ts">
  // La interfaz de Claude de un panel: aparece al escribir `claude` en su
  // terminal y se cierra con /exit (o Ctrl+D), volviendo al shell.
  import { onDestroy, onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { Attachment } from "$lib/session.svelte";
  import { prefs, TERMINALS, THEMES } from "$lib/prefs.svelte";
  import { updater } from "$lib/updater.svelte";
  import { keymap, ACTION_LABELS, displayAccel } from "$lib/keys.svelte";
  import { ws, type Pane } from "$lib/workspace.svelte";
  import { modelName, shortPath } from "$lib/format";
  import Message from "./Message.svelte";
  import Spinner from "./Spinner.svelte";
  import Permission from "./Permission.svelte";
  import Composer from "./Composer.svelte";
  import SessionPicker from "./SessionPicker.svelte";
  import Setup, { type ClaudeStatus } from "./Setup.svelte";

  let { pane, active, onexit }: { pane: Pane; active: boolean; onexit: () => void } = $props();

  // svelte-ignore state_referenced_locally
  const s = pane.session;
  let attachments = $state<Attachment[]>([]);
  let picker = $state(false);
  let root = $state<HTMLDivElement>();
  let scroller = $state<HTMLDivElement>();
  let composer = $state<ReturnType<typeof Composer>>();
  let stick = true;
  let appVersion = $state("");
  /** Estado de Claude Code si hace falta el asistente de primer arranque. */
  let setup = $state<ClaudeStatus | null>(null);
  getVersion().then((v) => (appVersion = v));

  const CWD_KEY = "ci.cwd";

  onMount(() => {
    (async () => {
      s.home = ws.home;
      // Sin Claude Code instalado o sin sesión iniciada, primero el asistente.
      const status = await invoke<ClaudeStatus>("claude_status");
      if (!status.installed || status.loggedIn === false) {
        setup = status;
        return;
      }
      await launch();
    })();
  });

  onDestroy(() => {
    s.stop();
  });

  async function launch() {
    const l = pane.launch ?? {};
    const cwd = pane.cwd;
    let resume = l.resume || undefined;
    if (l.continue) {
      const list = await invoke<{ id: string }[] | null>("list_sessions", { cwd }).catch(() => null);
      resume = list?.[0]?.id;
    }
    await startIn(cwd, resume);
    if (l.picker) picker = true;
    if (l.prompt) await submit(l.prompt);
  }

  // Ctrl+C como en el TUI: interrumpe a Claude, borra lo escrito o, pulsado
  // dos veces con el cuadro vacío, sale a la terminal.
  let ctrlCAt = 0;
  export function ctrlC(): boolean {
    if (setup || s.permission) return false;
    if (s.busy) {
      s.interrupt();
      return true;
    }
    if (composer?.clear()) return true;
    if (Date.now() - ctrlCAt < 2000) {
      ctrlCAt = 0;
      composer?.flash("");
      onexit();
    } else {
      ctrlCAt = Date.now();
      composer?.flash("Pulsa Ctrl+C otra vez para salir");
    }
    return true;
  }

  /** Foco al permiso pendiente si lo hay; si no, al cuadro de texto. */
  export function focus() {
    const perm = root?.querySelector<HTMLElement>(".perm, .setup");
    if (perm) perm.focus();
    else composer?.focus();
  }

  async function startIn(cwd: string, resume?: string) {
    try {
      localStorage.setItem(CWD_KEY, cwd);
    } catch {}
    try {
      await s.start({ cwd, resume });
    } catch (e) {
      s.notice(String(e), "error");
    }
    await tick();
    scrollToEnd(true);
    if (active) focus();
  }

  export async function addPaths(paths: string[]) {
    for (const path of paths) {
      if (attachments.some((a) => a.path === path)) continue;
      try {
        attachments = [...attachments, await invoke<Attachment>("read_attachment", { path })];
      } catch (e) {
        s.notice(String(e), "error");
      }
    }
    composer?.focus();
  }

  export async function pickFiles() {
    const r = await open({ multiple: true, directory: false, defaultPath: s.cwd });
    if (r) addPaths(Array.isArray(r) ? r : [r]);
  }

  async function changeDir() {
    const r = await open({ directory: true, defaultPath: s.cwd });
    if (typeof r === "string") await startIn(r);
  }

  export async function exportMd() {
    const path = await save({
      defaultPath: `${s.cwd}/conversacion-${new Date().toISOString().slice(0, 16).replace(/[:T]/g, "-")}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!path) return;
    await invoke("write_text_file", { path, content: s.toMarkdown() });
    s.notice(`Exportado a ${shortPath(path, s.home)}`, "info");
  }

  function keysHelp(): string {
    const rows = ACTION_LABELS.map(([a, label]) => {
      const keys = keymap.bindings[a];
      return keys?.length ? `  ${keys.map(displayAccel).join(", ").padEnd(28)} ${label}` : null;
    }).filter(Boolean);
    const from = keymap.terminal ? `de ${keymap.terminal}` : "del sistema";
    return `Atajos ${from} (se cambian en la configuración de tu terminal; /terminal elige cuál imitar):\n${rows.join("\n")}`;
  }

  async function submit(text: string) {
    const cmd = text.match(/^\/(\S+)\s*([\s\S]*)$/);
    if (cmd && !attachments.length) {
      const [, name, arg] = cmd;
      switch (name) {
        case "clear":
        case "new":
          return startIn(s.cwd);
        case "resume":
          picker = true;
          return;
        case "cd":
          return arg ? startIn(arg.replace(/^~/, s.home)) : changeDir();
        case "export":
          return exportMd();
        case "exit":
        case "quit":
          return onexit();
        case "keys":
          return s.notice(keysHelp());
        case "copy": {
          const last = [...s.items].reverse().find((i) => i.kind === "text");
          if (last?.kind === "text") await writeText(last.text);
          return s.notice("Última respuesta copiada al portapapeles", "info");
        }
        case "model":
          if (arg) return s.setModel(arg.trim());
          return s.notice(`Modelo actual: ${modelName(s.model) || s.modelPref} · usa /model <opus|sonnet|haiku|id>`);
        case "font": {
          const a = arg.trim();
          if (!a) {
            return s.notice(
              `Fuente: ${prefs.fontFamily ?? "monoespaciada del sistema"} · ${prefs.fontSize}px${prefs.config.fontFamily || prefs.config.fontSize ? "" : prefs.terminal ? ` (de ${prefs.terminal.terminal})` : ""} · usa /font <familia>, /font <tamaño> o /font reset (también ctrl + / ctrl - / ctrl 0)`,
            );
          }
          if (a === "reset") await prefs.set({ fontFamily: undefined, fontSize: undefined });
          else if (/^\d+$/.test(a)) await prefs.set({ fontSize: Math.min(28, Math.max(9, Number(a))) });
          else await prefs.set({ fontFamily: a.replace(/^["']|["']$/g, "") });
          return s.notice(`Fuente: ${prefs.fontFamily ?? "monoespaciada del sistema"} · ${prefs.fontSize}px`);
        }
        case "terminal": {
          const a = arg.trim();
          if (!a) {
            const found = await invoke<string[]>("detected_terminals");
            const t = prefs.terminal;
            const now = t ? `${t.terminal}${t.profile ? ` (${t.profile})` : ""}${t.fontFamily ? ` · ${t.fontFamily} ${t.fontSize ?? ""}pt` : ""}` : "ninguno";
            return s.notice(
              `Estilo de terminal: ${now} · modo ${prefs.config.terminal ?? "auto"}\nDetectadas: ${found.join(", ") || "ninguna"}\nUsa /terminal <auto|off|${TERMINALS.join("|")}>`,
            );
          }
          if (a !== "auto" && a !== "off" && !TERMINALS.includes(a as any)) return s.notice(`Terminal desconocida: ${a}`, "error");
          await prefs.set({ terminal: a === "auto" ? undefined : a });
          if (a !== "off" && !prefs.terminal) return s.notice(`No encontré configuración de ${a}`, "warn");
          return s.notice(prefs.terminal ? `Estilo de ${prefs.terminal.terminal}${prefs.terminal.profile ? ` (${prefs.terminal.profile})` : ""}` : "Estilo de terminal desactivado");
        }
        case "update": {
          const a = arg.trim();
          if (a === "off" || a === "on") {
            await prefs.set({ autoUpdate: a === "on" ? undefined : false });
            await updater.start(a === "on");
            return s.notice(a === "on" ? "Actualizaciones automáticas activadas" : "Actualizaciones automáticas desactivadas");
          }
          if (updater.kind === "dev") return s.notice("Build de desarrollo: las actualizaciones están desactivadas", "warn");
          s.notice("Buscando actualizaciones…");
          const found = await updater.check();
          if (updater.status === "error") return s.notice(`No se pudo comprobar: ${updater.error}`, "error");
          return s.notice(found ? `Nueva versión v${updater.version} disponible` : `Ya tienes la última versión (v${appVersion})`);
        }
        case "theme": {
          const a = arg.trim();
          if (!a) {
            const origin = prefs.config.theme ? "elegido en la app" : "de Claude Code";
            return s.notice(`Tema: ${prefs.theme} (${origin}) · usa /theme <${THEMES.join("|")}|claude>`);
          }
          if (a === "claude") {
            await prefs.set({ theme: undefined });
            return s.notice(`Tema: ${prefs.theme} (sigue a Claude Code)`);
          }
          if (!THEMES.includes(a as any)) return s.notice(`Tema desconocido: ${a}`, "error");
          await prefs.set({ theme: a });
          return s.notice(`Tema: ${prefs.theme}`);
        }
        case "effort":
          if (arg) return s.setEffort(arg.trim());
          return s.notice(`Esfuerzo actual: ${s.effort || "por defecto"} · usa /effort <low|medium|high|xhigh|max>`);
      }
    }
    if (text.startsWith("!") && !attachments.length) {
      await s.runShell(text.slice(1).trim());
      return scrollToEnd(true);
    }
    const att = attachments;
    attachments = [];
    await s.send(text, att);
    scrollToEnd(true);
  }

  function scrollToEnd(force = false) {
    const el = scroller;
    if (!el || (!force && !stick)) return;
    requestAnimationFrame(() => (el.scrollTop = el.scrollHeight));
  }

  function onScroll() {
    const el = scroller;
    if (el) stick = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
  }

  $effect(() => {
    void s.items.length;
    void s.live?.text;
    void s.permission;
    void s.busy;
    scrollToEnd();
  });

  async function onClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    const copyBtn = target.closest("[data-copy]") as HTMLElement | null;
    if (copyBtn) {
      const code = copyBtn.parentElement?.querySelector("code")?.textContent ?? "";
      await writeText(code);
      copyBtn.textContent = "✓ copiado";
      setTimeout(() => (copyBtn.textContent = "copiar"), 1200);
      return;
    }
    const a = target.closest("a[href]") as HTMLAnchorElement | null;
    if (a && /^https?:/.test(a.href)) {
      e.preventDefault();
      openUrl(a.href);
      return;
    }
    if (window.getSelection()?.isCollapsed && !target.closest("input, textarea, button, a, .perm, [tabindex]")) focus();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions, a11y_no_static_element_interactions -->
<div class="chat" bind:this={root} onclick={onClick}>
  {#if setup}
    <div class="scroll">
      <Setup
        status={setup}
        onready={() => {
          setup = null;
          launch();
        }}
      />
    </div>
  {:else}
  <div class="scroll" bind:this={scroller} onscroll={onScroll}>
    <div class="transcript">
      <header class="welcome">
        <div class="legend"><span class="claude">✻</span> Claude Interface{appVersion ? ` v${appVersion}` : ""} <span class="faint">· Claude Code{s.version ? ` v${s.version}` : ""}</span></div>
        <div class="cols">
          <div class="logo" aria-hidden="true">
            <pre>{` ▐▛███▜▌\n▝▜█████▛▘\n  ▘▘ ▝▝`}</pre>
          </div>
          <div class="info">
            <div class="bold">¡Bienvenido de vuelta!</div>
            <div class="dim">{modelName(s.model) || s.modelPref || "modelo por defecto"} · {s.effort || "effort por defecto"}</div>
            <button class="cwd" onclick={changeDir} title="Cambiar directorio">{shortPath(s.cwd, s.home)}</button>
          </div>
          <div class="tips">
            <div class="claude">Atajos</div>
            <div><span class="k">arrastra</span> o <span class="k">ctrl+o</span> adjunta archivos · <span class="k">ctrl+v</span> pega imágenes</div>
            <div><span class="k">/resume</span> retomar · <span class="k">/export</span> a .md · <span class="k">/cd</span> cambiar carpeta</div>
            <div><span class="k">!cmd</span> ejecuta en shell local (sin tokens) · <span class="k">/font</span> <span class="k">/theme</span> <span class="k">/terminal</span> <span class="k">ctrl ±</span> tamaño</div>
            <div><span class="k">{keymap.label("new_tab") ?? "/keys"}</span> pestaña · <span class="k">{keymap.label("split_right") ?? ""}</span> <span class="k">{keymap.label("split_down") ?? ""}</span> dividir · <span class="k">/keys</span> todos los atajos</div>
          </div>
        </div>
      </header>

      {#each s.items as item (item.id)}
        <Message {item} cwd={s.cwd} />
      {/each}

      {#if s.live?.text}
        {#if s.live.kind === "text"}
          <Message item={{ kind: "text", id: -1, text: s.live.text }} cwd={s.cwd} />
        {:else}
          <div class="live-thinking">✻ Thinking… <span class="faint">{s.live.text.slice(-160)}</span></div>
        {/if}
      {/if}

      {#if s.permission}
        {#key s.permission.requestId}
          <Permission {s} req={s.permission} autofocus={active} />
        {/key}
      {:else if s.busy}
        <Spinner since={s.turnStart} tokens={s.thinkingTokens} />
      {/if}

      {#if !s.running && s.stderr.length}
        <div class="stderr">
          <div class="bold">claude terminó:</div>
          <pre>{s.stderr.join("\n")}</pre>
        </div>
      {/if}

      {#if picker}
        <SessionPicker
          cwd={s.cwd}
          onclose={() => {
            picker = false;
            composer?.focus();
          }}
          onpick={(id) => {
            picker = false;
            startIn(s.cwd, id);
          }}
        />
      {/if}
    </div>
  </div>

  <div class="bottom">
    <div class="transcript">
      
    </div>
    <div class="transcript">
      <Composer
        bind:this={composer}
        {s}
        bind:attachments
        onsubmit={submit}
        onattach={pickFiles}
        onpastepaths={addPaths}
        {onexit}
      />
    </div>
  </div>
  {/if}
</div>

<style>
  .chat {
    position: absolute;
    inset: 0;
    background: var(--bg);
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding-top: 1em;
  }
  .transcript {
    max-width: 1100px;
    margin: 0 auto;
    padding: 0 var(--gutter);
  }
  .bottom .transcript {
    padding: 0;
  }
  .bottom {
    flex: none;
    padding-top: 0.4em;
  }
  .claude {
    color: var(--claude);
  }
  .welcome {
    position: relative;
    border: 1px solid var(--claude);
    border-radius: 8px;
    padding: 1em 2ch 0.8em;
    margin: 0.6em 0 1.2em;
  }
  .legend {
    position: absolute;
    top: -0.75em;
    left: 2ch;
    background: var(--bg);
    padding: 0 1ch;
  }
  .cols {
    display: flex;
    gap: 3ch;
    align-items: center;
    flex-wrap: wrap;
  }
  .logo pre {
    margin: 0;
    color: var(--claude);
    font-family: var(--font);
    line-height: 1.1;
  }
  .info {
    display: flex;
    flex-direction: column;
    min-width: 20ch;
  }
  .cwd {
    text-align: left;
    color: var(--text-dim);
    text-decoration: underline dotted var(--text-faint);
  }
  .cwd:hover {
    color: var(--claude);
  }
  .tips {
    flex: 1;
    border-left: 1px solid var(--claude);
    padding-left: 2ch;
    font-size: 0.93em;
    color: var(--text-dim);
    min-width: 30ch;
  }
  .k {
    color: var(--text);
  }
  .live-thinking {
    color: var(--text-dim);
    font-style: italic;
    margin: 0.9em 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .stderr {
    color: var(--error);
    margin: 1em 0;
  }
  .stderr pre {
    white-space: pre-wrap;
    font-family: var(--font);
    font-size: 0.86em;
  }
</style>
