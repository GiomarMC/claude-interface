<script lang="ts">
  import { fmtBytes } from "$lib/format";
  import type { Attachment, Session, SlashCommand } from "$lib/session.svelte";
  import StatusLine from "./StatusLine.svelte";

  let {
    s,
    attachments = $bindable(),
    onsubmit,
    onattach,
    onpastepaths,
  }: {
    s: Session;
    attachments: Attachment[];
    onsubmit: (text: string) => void;
    onattach: () => void;
    onpastepaths: (paths: string[]) => void;
  } = $props();

  let text = $state("");
  let el: HTMLTextAreaElement;
  let menuIndex = $state(0);

  const HISTORY_KEY = "ci.history";
  let history: string[] = load();
  let historyPos = -1;

  function load(): string[] {
    try {
      return JSON.parse(localStorage.getItem(HISTORY_KEY) ?? "[]");
    } catch {
      return [];
    }
  }

  export function focus() {
    el?.focus();
  }

  export function insert(t: string) {
    text = t;
    setTimeout(() => {
      el.focus();
      el.setSelectionRange(text.length, text.length);
      autosize();
    });
  }

  const slashQuery = $derived(/^\/[^\s]*$/.test(text) ? text.slice(1).toLowerCase() : null);
  const matches: SlashCommand[] = $derived(
    slashQuery == null
      ? []
      : s.commands
          .filter((c) => c.name.toLowerCase().includes(slashQuery))
          .sort((a, b) => Number(!a.name.startsWith(slashQuery)) - Number(!b.name.startsWith(slashQuery)))
          .slice(0, 8),
  );

  $effect(() => {
    void matches.length;
    menuIndex = 0;
  });

  function autosize() {
    if (!el) return;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, window.innerHeight * 0.4) + "px";
  }

  function submit() {
    const t = text.trim();
    if (!t && !attachments.length) return;
    if (t) {
      history = [t, ...history.filter((h) => h !== t)].slice(0, 200);
      try {
        localStorage.setItem(HISTORY_KEY, JSON.stringify(history));
      } catch {}
    }
    historyPos = -1;
    text = "";
    setTimeout(autosize);
    onsubmit(t);
  }

  function complete(c: SlashCommand) {
    text = `/${c.name} `;
    el.focus();
  }

  function onKey(e: KeyboardEvent) {
    if (matches.length) {
      if (e.key === "ArrowDown") {
        menuIndex = (menuIndex + 1) % matches.length;
        return e.preventDefault();
      }
      if (e.key === "ArrowUp") {
        menuIndex = (menuIndex - 1 + matches.length) % matches.length;
        return e.preventDefault();
      }
      if (e.key === "Tab") {
        complete(matches[menuIndex]);
        return e.preventDefault();
      }
      if (e.key === "Enter" && !e.shiftKey) {
        const c = matches[menuIndex];
        if (`/${c.name}` !== text.trim() && (c.argumentHint || c.name === "model" || c.name === "effort")) {
          complete(c);
          return e.preventDefault();
        }
        text = `/${c.name}`;
      }
    }
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      submit();
    } else if (e.key === "Tab" && e.shiftKey) {
      e.preventDefault();
      s.cycleMode();
    } else if (e.key === "Escape") {
      // Con Claude trabajando, el atajo global de la ventana interrumpe.
      if (!s.busy && text) text = "";
    } else if (e.key === "ArrowUp" && (text === "" || historyPos >= 0) && !text.includes("\n")) {
      if (historyPos < history.length - 1) {
        historyPos++;
        text = history[historyPos];
        e.preventDefault();
      }
    } else if (e.key === "ArrowDown" && historyPos >= 0) {
      historyPos--;
      text = historyPos >= 0 ? history[historyPos] : "";
      e.preventDefault();
    } else if (e.key === "Backspace" && text === "" && attachments.length) {
      attachments = attachments.slice(0, -1);
    }
  }

  async function onPaste(e: ClipboardEvent) {
    const dt = e.clipboardData;
    if (!dt) return;
    const uris = dt.getData("text/uri-list");
    if (uris && uris.includes("file://")) {
      e.preventDefault();
      onpastepaths(
        uris
          .split(/\r?\n/)
          .filter((u) => u.startsWith("file://"))
          .map((u) => decodeURIComponent(new URL(u).pathname)),
      );
      return;
    }
    const images = [...dt.items].filter((i) => i.kind === "file" && i.type.startsWith("image/"));
    if (!images.length) return;
    e.preventDefault();
    for (const item of images) {
      const file = item.getAsFile();
      if (!file) continue;
      const data = await new Promise<string>((res) => {
        const r = new FileReader();
        r.onload = () => res(String(r.result).split(",")[1] ?? "");
        r.readAsDataURL(file);
      });
      const n = attachments.filter((a) => a.name.startsWith("Imagen pegada")).length + 1;
      attachments = [
        ...attachments,
        { name: `Imagen pegada #${n}`, path: "", kind: "image", mime: file.type, size: file.size, data },
      ];
    }
  }

  const modeInfo = $derived(
    ({
      default: null,
      acceptEdits: { label: "⏵⏵ accept edits on", color: "var(--auto-accept)" },
      plan: { label: "⏸ plan mode on", color: "var(--plan)" },
      auto: { label: "⏵⏵ auto mode on", color: "var(--warning)" },
      bypassPermissions: { label: "⏵⏵ bypass permissions on", color: "var(--error)" },
    } as const)[s.mode],
  );
</script>

<div class="composer">
  {#if attachments.length}
    <div class="atts">
      {#each attachments as a, i}
        <span class="att" title={a.path || a.name}>
          {#if a.kind === "image"}
            <img src={`data:${a.mime};base64,${a.data}`} alt={a.name} />
          {:else}
            <span class="icon">{a.kind === "dir" ? "▤" : a.kind === "pdf" ? "▥" : "◧"}</span>
          {/if}
          <span class="aname">{a.name}</span>
          <span class="faint">
            {a.kind === "ref" || a.kind === "dir" ? (a.note ?? "por ruta") : fmtBytes(a.size)}
          </span>
          <button class="x" onclick={() => (attachments = attachments.filter((_, j) => j !== i))} title="Quitar">×</button>
        </span>
      {/each}
    </div>
  {/if}

  <div class="box" class:bash={text.startsWith("!")}>
    <span class="prompt">{text.startsWith("!") ? "!" : ">"}</span>
    <textarea
      bind:this={el}
      bind:value={text}
      rows="1"
      spellcheck="false"
      placeholder={s.busy ? "Escribe para encolar un mensaje…" : 'Try "explica este proyecto" · arrastra archivos o pega imágenes'}
      oninput={autosize}
      onkeydown={onKey}
      onpaste={onPaste}
    ></textarea>
    <button class="clip" onclick={onattach} title="Adjuntar archivos">⊕</button>
  </div>

  {#if matches.length}
    <div class="menu">
      {#each matches as c, i}
        <button class="cmd" class:sel={i === menuIndex} onmouseenter={() => (menuIndex = i)} onclick={() => complete(c)}>
          <span class="cname">/{c.name}{c.argumentHint ? ` ${c.argumentHint}` : ""}</span>
          <span class="cdesc">{c.description}</span>
        </button>
      {/each}
    </div>
  {/if}

  <StatusLine {s} />

  <div class="hints">
    {#if modeInfo}
      <button style:color={modeInfo.color} onclick={() => s.cycleMode()}>
        {modeInfo.label} <span class="faint">(shift+tab to cycle)</span>
      </button>
    {:else}
      <button class="faint" onclick={() => s.cycleMode()}>? shift+tab: modos · / comandos · ↑ historial · esc interrumpir</button>
    {/if}
  </div>
</div>

<style>
  .composer {
    padding: 0 var(--gutter) 0.6em;
  }
  .atts {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5em 1ch;
    margin-bottom: 0.4em;
  }
  .att {
    display: inline-flex;
    align-items: center;
    gap: 0.8ch;
    border: 1px solid var(--border-faint);
    border-radius: 6px;
    padding: 0.1em 0.8ch;
    font-size: 0.93em;
    color: var(--suggestion);
    max-width: 48ch;
    white-space: nowrap;
  }
  .att img {
    height: 22px;
    width: 22px;
    object-fit: cover;
    border-radius: 3px;
  }
  .aname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .x {
    color: var(--text-faint);
    padding-left: 0.3ch;
  }
  .x:hover {
    color: var(--error);
  }
  .box {
    display: flex;
    align-items: flex-start;
    gap: 1ch;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.35em 1ch;
  }
  .box:focus-within {
    border-color: var(--text-dim);
  }
  .box.bash {
    border-color: var(--bash);
  }
  .box.bash .prompt {
    color: var(--bash);
  }
  .prompt {
    color: var(--text);
    flex: none;
  }
  textarea {
    flex: 1;
    resize: none;
    background: none;
    border: none;
    outline: none;
    color: var(--text);
    font: inherit;
    line-height: var(--line);
    padding: 0;
    max-height: 40vh;
  }
  textarea::placeholder {
    color: var(--text-faint);
  }
  .clip {
    color: var(--text-faint);
    flex: none;
  }
  .clip:hover {
    color: var(--claude);
  }
  .menu {
    display: flex;
    flex-direction: column;
    padding: 0.3em 1ch;
  }
  .cmd {
    display: grid;
    grid-template-columns: minmax(18ch, 28ch) 1fr;
    gap: 2ch;
    text-align: left;
    color: var(--text-dim);
  }
  .cmd.sel .cname {
    color: var(--suggestion);
  }
  .cmd.sel .cdesc {
    color: var(--text);
  }
  .cname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cdesc {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hints {
    padding: 0.2em 1ch 0;
    font-size: 0.93em;
  }
</style>
