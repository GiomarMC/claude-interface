<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { timeAgo } from "$lib/format";

  type Summary = { id: string; title: string; modified: number; messages: number };

  let { cwd, onpick, onclose }: { cwd: string; onpick: (id: string) => void; onclose: () => void } = $props();

  let sessions = $state<Summary[] | null>(null);
  let filter = $state("");
  let selected = $state(0);
  let input: HTMLInputElement;

  const shown = $derived(
    (sessions ?? []).filter((x) => x.title.toLowerCase().includes(filter.toLowerCase())),
  );

  onMount(async () => {
    input.focus();
    sessions = await invoke<Summary[]>("list_sessions", { cwd });
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
    else if (e.key === "ArrowDown") selected = Math.min(shown.length - 1, selected + 1);
    else if (e.key === "ArrowUp") selected = Math.max(0, selected - 1);
    else if (e.key === "Enter" && shown[selected]) onpick(shown[selected].id);
    else return;
    e.preventDefault();
  }
</script>

<div class="picker" role="dialog" aria-label="Retomar conversación">
  <div class="title bold">Resume a conversation</div>
  <div class="search">
    <span class="dim">⌕</span>
    <input bind:this={input} bind:value={filter} onkeydown={onKey} oninput={() => (selected = 0)} placeholder="Buscar…" />
  </div>
  <div class="list">
    {#if sessions === null}
      <div class="dim">Cargando…</div>
    {:else if !shown.length}
      <div class="dim">No hay conversaciones en este directorio.</div>
    {/if}
    {#each shown as x, i}
      <button class="row" class:sel={i === selected} onmouseenter={() => (selected = i)} onclick={() => onpick(x.id)}>
        <span class="cur">{i === selected ? "❯" : " "}</span>
        <span class="t">{x.title}</span>
        <span class="faint meta">{timeAgo(x.modified)} · {x.messages} msgs</span>
      </button>
    {/each}
  </div>
  <div class="faint hint">↑/↓ navegar · enter retomar · esc cerrar</div>
</div>

<style>
  .picker {
    border: 1px solid var(--claude);
    border-radius: 8px;
    padding: 0.6em 2ch;
    margin: 0.8em 0;
    background: var(--bg);
  }
  .title {
    color: var(--claude);
  }
  .search {
    display: flex;
    gap: 1ch;
    margin: 0.4em 0;
  }
  input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: var(--text);
    font: inherit;
  }
  .list {
    max-height: 50vh;
    overflow: auto;
  }
  .row {
    display: flex;
    gap: 1ch;
    width: 100%;
    text-align: left;
    color: var(--text);
  }
  .row.sel {
    color: var(--suggestion);
  }
  .cur {
    width: 1ch;
    flex: none;
  }
  .t {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    flex: none;
    font-size: 12px;
  }
  .hint {
    font-size: 12px;
    margin-top: 0.4em;
  }
</style>
