<script lang="ts">
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { renderMarkdown } from "$lib/markdown";
  import { fmtBytes } from "$lib/format";
  import type { Item } from "$lib/session.svelte";
  import ToolCall from "./ToolCall.svelte";

  let { item, cwd }: { item: Item; cwd: string } = $props();
  let copied = $state(false);
  let showThinking = $state(false);

  async function copy(text: string) {
    await writeText(text);
    copied = true;
    setTimeout(() => (copied = false), 1200);
  }
</script>

{#if item.kind === "user"}
  <div class="user">
    <span class="prompt">&gt;</span>
    <div class="utext">
      <span class="ut">{item.text}</span>
      {#if item.attachments.length}
        <div class="chips">
          {#each item.attachments as a}
            <span class="chip" title={a.path}>
              {a.kind === "image" ? "▣" : a.kind === "dir" ? "▤" : "◧"} {a.name}
              <span class="faint">{a.kind === "ref" || a.kind === "dir" ? "ruta" : fmtBytes(a.size)}</span>
            </span>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{:else if item.kind === "text"}
  <div class="assistant">
    <span class="bullet">●</span>
    <div class="md">{@html renderMarkdown(item.text)}</div>
    <button class="copy" onclick={() => copy(item.text)} title="Copiar como Markdown">
      {copied ? "✓ copiado" : "⧉ copiar"}
    </button>
  </div>
{:else if item.kind === "thinking"}
  <div class="thinking">
    <button class="tlabel" onclick={() => (showThinking = !showThinking)}>
      ✻ Thinking… <span class="faint">{showThinking ? "(ocultar)" : "(ver)"}</span>
    </button>
    {#if showThinking}
      <div class="tbody md">{@html renderMarkdown(item.text)}</div>
    {/if}
  </div>
{:else if item.kind === "tool"}
  <ToolCall {item} {cwd} />
{:else if item.kind === "system"}
  <div class="system {item.tone}">
    <span class="elbow">⎿</span>
    <span class="stext">{item.text}</span>
  </div>
{/if}

<style>
  .user {
    display: flex;
    gap: 1ch;
    background: var(--bg-user);
    color: var(--text-dim);
    padding: 0.35em 1ch;
    margin: 1em 0 0.6em;
    border-radius: 2px;
  }
  .prompt {
    flex: none;
  }
  .ut {
    white-space: pre-wrap;
  }
  .utext {
    overflow-wrap: anywhere;
    min-width: 0;
    color: var(--text);
    opacity: 0.85;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5ch 1ch;
    margin-top: 0.3em;
  }
  .chip {
    color: var(--suggestion);
    font-size: 0.93em;
  }
  .assistant {
    position: relative;
    display: flex;
    gap: 1ch;
    margin: 0.9em 0;
  }
  .bullet {
    flex: none;
    color: var(--text);
  }
  .md {
    min-width: 0;
    flex: 1;
  }
  .copy {
    position: absolute;
    top: -1.3em;
    right: 0;
    font-size: 0.86em;
    color: var(--text-faint);
    opacity: 0;
    transition: opacity 0.12s;
  }
  .assistant:hover .copy {
    opacity: 1;
  }
  .copy:hover {
    color: var(--claude);
  }
  .thinking {
    margin: 0.9em 0;
    color: var(--text-dim);
    font-style: italic;
  }
  .tlabel {
    font-style: italic;
    color: var(--text-dim);
  }
  .tbody {
    padding-left: 2ch;
    border-left: 1px solid var(--border-faint);
    margin-top: 0.3em;
  }
  .system {
    display: flex;
    gap: 2ch;
    padding-left: 2ch;
    margin: 0.5em 0;
    color: var(--text-dim);
  }
  .system.error {
    color: var(--error);
  }
  .system.warn {
    color: var(--warning);
  }
  .stext {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
