<script lang="ts">
  import "../app.css";
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { homeDir } from "@tauri-apps/api/path";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { Session, type Attachment } from "$lib/session.svelte";
  import { modelName, shortPath } from "$lib/format";
  import Message from "$lib/components/Message.svelte";
  import Spinner from "$lib/components/Spinner.svelte";
  import Permission from "$lib/components/Permission.svelte";
  import Composer from "$lib/components/Composer.svelte";
  import SessionPicker from "$lib/components/SessionPicker.svelte";

  const s = new Session();
  let attachments = $state<Attachment[]>([]);
  let dragging = $state(false);
  let picker = $state(false);
  let scroller: HTMLDivElement;
  let composer: ReturnType<typeof Composer>;
  let stick = true;

  const CWD_KEY = "ci.cwd";

  onMount(() => {
    let unlistenDrop: (() => void) | undefined;
    (async () => {
      await s.init();
      s.home = (await homeDir()).replace(/\/$/, "");
      let last: string | null = null;
      try {
        last = localStorage.getItem(CWD_KEY);
      } catch {}
      const cwd = (await invoke<string | null>("initial_cwd")) ?? last ?? s.home;
      await startIn(cwd);

      unlistenDrop = await getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "enter" || p.type === "over") dragging = true;
        else if (p.type === "leave") dragging = false;
        else if (p.type === "drop") {
          dragging = false;
          addPaths(p.paths);
        }
      });
    })();
    return () => {
      unlistenDrop?.();
      s.stop();
    };
  });

  async function startIn(cwd: string, resume?: string) {
    try {
      localStorage.setItem(CWD_KEY, cwd);
    } catch {}
    try {
      await s.start({ cwd, resume });
    } catch (e) {
      s.notice(String(e), "error");
    }
    getCurrentWindow().setTitle(`Claude · ${cwd.split("/").pop() || cwd}`);
    await tick();
    scrollToEnd(true);
    composer?.focus();
  }

  async function addPaths(paths: string[]) {
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

  async function pickFiles() {
    const r = await open({ multiple: true, directory: false, defaultPath: s.cwd });
    if (r) addPaths(Array.isArray(r) ? r : [r]);
  }

  async function changeDir() {
    const r = await open({ directory: true, defaultPath: s.cwd });
    if (typeof r === "string") await startIn(r);
  }

  async function exportMd() {
    const path = await save({
      defaultPath: `${s.cwd}/conversacion-${new Date().toISOString().slice(0, 16).replace(/[:T]/g, "-")}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!path) return;
    await invoke("write_text_file", { path, content: s.toMarkdown() });
    s.notice(`Exportado a ${shortPath(path, s.home)}`, "info");
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
        case "copy": {
          const last = [...s.items].reverse().find((i) => i.kind === "text");
          if (last?.kind === "text") await writeText(last.text);
          return s.notice("Última respuesta copiada al portapapeles", "info");
        }
        case "model":
          if (arg) return s.setModel(arg.trim());
          return s.notice(`Modelo actual: ${modelName(s.model) || s.modelPref} · usa /model <opus|sonnet|haiku|id>`);
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
    if (!scroller || (!force && !stick)) return;
    requestAnimationFrame(() => (scroller.scrollTop = scroller.scrollHeight));
  }

  function onScroll() {
    stick = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 80;
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
    }
  }

  function onGlobalKey(e: KeyboardEvent) {
    if (e.key === "Escape" && s.busy && !s.permission) s.interrupt();
    else if (e.ctrlKey && e.key.toLowerCase() === "o") {
      e.preventDefault();
      pickFiles();
    } else if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "e") {
      e.preventDefault();
      exportMd();
    }
  }
</script>

<svelte:window onkeydown={onGlobalKey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<main onclick={onClick}>
  <div class="scroll" bind:this={scroller} onscroll={onScroll}>
    <div class="transcript">
      <header class="welcome">
        <div class="legend"><span class="claude">✻</span> Claude Interface <span class="faint">· Claude Code {s.version ? `v${s.version}` : ""}</span></div>
        <div class="cols">
          <div class="logo" aria-hidden="true">
            <pre>{` ▐▛███▜▌\n▝▜█████▛▘\n  ▘▘ ▝▝`}</pre>
          </div>
          <div class="info">
            <div class="bold">¡Bienvenido de vuelta!</div>
            <div class="dim">{modelName(s.model) || s.modelPref || "—"} · {s.effort || "effort por defecto"}</div>
            <button class="cwd" onclick={changeDir} title="Cambiar directorio">{shortPath(s.cwd, s.home)}</button>
          </div>
          <div class="tips">
            <div class="claude">Atajos</div>
            <div><span class="k">arrastra</span> o <span class="k">ctrl+o</span> adjunta archivos · <span class="k">ctrl+v</span> pega imágenes</div>
            <div><span class="k">/resume</span> retomar · <span class="k">/export</span> a .md · <span class="k">/cd</span> cambiar carpeta</div>
            <div><span class="k">!cmd</span> ejecuta en shell local (sin tokens)</div>
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
          <Permission {s} req={s.permission} />
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
      <Composer
        bind:this={composer}
        {s}
        bind:attachments
        onsubmit={submit}
        onattach={pickFiles}
        onpastepaths={addPaths}
      />
    </div>
  </div>

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
    font-size: 13px;
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
    font-size: 12px;
  }
  .drop {
    position: absolute;
    inset: 0;
    background: rgba(20, 20, 19, 0.85);
    display: grid;
    place-items: center;
    pointer-events: none;
  }
  .drop-box {
    border: 1px dashed var(--claude);
    border-radius: 10px;
    padding: 2em 4ch;
    text-align: center;
  }
</style>
