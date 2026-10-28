<script lang="ts">
  // Ejecuta el `statusLine.command` del usuario, como Claude Code. Si no
  // tiene uno configurado, muestra un statusline sencillo por defecto.
  import { invoke } from "@tauri-apps/api/core";
  import { onDestroy } from "svelte";
  import { ansiToHtml } from "$lib/ansi";
  import { barColor, fmtNum, modelName } from "$lib/format";
  import { prefs } from "$lib/prefs.svelte";
  import type { Session } from "$lib/session.svelte";

  let { s }: { s: Session } = $props();

  let lines = $state<string[]>([]);
  let failed = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let running = false;
  let again = false;

  async function run() {
    const command = s.statusLineCommand;
    if (!command || !s.cwd) return;
    if (running) {
      again = true;
      return;
    }
    running = true;
    try {
      const out = await invoke<string>("run_statusline", { command, input: s.statusLineInput(), cwd: s.cwd });
      lines = out.replace(/\n+$/, "").split("\n").map(ansiToHtml);
      failed = false;
    } catch {
      failed = true;
    } finally {
      running = false;
      if (again) {
        again = false;
        schedule();
      }
    }
  }

  function schedule() {
    clearTimeout(timer);
    timer = setTimeout(run, 300);
  }

  // Igual que Claude Code: se refresca cuando cambia el estado de la sesión.
  $effect(() => {
    void [
      s.statusLineCommand, s.cwd, s.model, s.modelPref, s.mode, s.effort, s.contextUsed,
      s.contextMax, s.costUsd, s.fiveHour, s.sevenDay, s.busy, s.items.length, s.git,
      prefs.terminal, prefs.theme,
    ];
    schedule();
  });

  // Los contadores de reinicio cambian aunque no haya actividad.
  const tick = setInterval(() => s.statusLineCommand && !s.busy && run(), 30_000);
  onDestroy(() => {
    clearInterval(tick);
    clearTimeout(timer);
  });

  const model = $derived(modelName(s.model) || s.modelPref || "claude");
  const pct = $derived(Math.round(s.contextPct ?? 0));
</script>

<div class="sl">
  {#if s.statusLineCommand && !failed}
    {#each lines as l}
      <div class="ansi">{@html l}</div>
    {/each}
  {:else}
    <div class="row">
      <span class="bold">{model}</span>
      {#if s.git}
        <span class="dim">·</span>
        <span>{s.git.repo}{#if s.git.branch}<span class="dim"> ({s.git.branch})</span>{/if}</span>
      {/if}
      {#if s.contextMax}
        <span class="dim">·</span>
        <span><span style:color={barColor(pct)}>{pct}%</span> <span class="dim">contexto ({fmtNum(s.contextUsed ?? 0)} / {fmtNum(s.contextMax)})</span></span>
      {/if}
      {#if s.effort}
        <span class="dim">· {s.effort}</span>
      {/if}
    </div>
    {#if failed}
      <div class="faint">statusline: el comando falló o tardó más de 5 s</div>
    {/if}
  {/if}
</div>

<style>
  .sl {
    padding: 0.3em 1ch 0;
    font-size: calc(var(--font-size) - 1px);
    line-height: 1.5;
    min-height: 1.5em;
  }
  .ansi {
    white-space: pre;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    column-gap: 1ch;
    color: var(--text-dim);
  }
  .row .bold {
    color: var(--text);
  }
</style>
