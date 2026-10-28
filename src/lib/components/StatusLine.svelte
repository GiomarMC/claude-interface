<script lang="ts">
  // Réplica de ~/.claude/statusline-command.sh
  import { barColor, fmtCost, fmtMs, fmtNum, fmtReset, modelName } from "$lib/format";
  import type { Session } from "$lib/session.svelte";

  let { s }: { s: Session } = $props();

  const WIDTH = 40;
  const model = $derived(modelName(s.model) || s.modelPref || "claude");
  const pct = $derived(s.contextPct ?? 0);
  const filled = $derived(Math.min(WIDTH, Math.floor((pct / 100) * WIDTH)));
  const tip = $derived(filled < WIDTH);
  const empty = $derived(Math.max(0, WIDTH - filled - (tip ? 1 : 0)));
  const color = $derived(barColor(pct));
  const effortIcon = $derived(
    ({ low: "🟢", medium: "🟡", high: "🔴" } as Record<string, string>)[s.effort] ?? "🔒",
  );
</script>

<div class="sl">
  <div class="row">
    <span class="w bold">[{model}{s.contextMax ? ` (${fmtNum(s.contextMax)})` : ""}]</span>
    {#if s.thinking}<span>🧠</span>{/if}
    {#if s.fast}<span>⚡fast</span>{/if}
    {#if s.git}
      <span class="d">🖥</span>
      <span class="w">{s.git.repo}</span>
      {#if s.git.branch}
        <span class="d sep">│</span>
        <span class="w">🌿 {s.git.branch}</span>
        {#if s.git.insertions || s.git.deletions}
          <span><span class="d">+{s.git.insertions}</span> <span class="r">-{s.git.deletions}</span></span>
        {/if}
      {/if}
    {/if}
    {#if s.effort}
      <span class="d sep">│</span>
      <span>{effortIcon} <span class="d">{s.effort}</span></span>
    {/if}
  </div>

  {#if s.contextPct != null && s.contextMax}
    <div class="row bar">
      <span class="d">[</span><span class="bold" style:color>{"█".repeat(filled)}</span>{#if tip}<span class="d">▓</span>{/if}<span class="d">{"░".repeat(empty)}</span><span class="d">]</span>
      <span class="pct bold" style:color>{Math.round(pct)}%</span>
      <span class="d">{fmtNum(s.contextUsed ?? 0)} / {fmtNum(s.contextMax)}</span>
    </div>
  {/if}

  {#if s.fiveHour || s.sevenDay || s.costUsd != null || s.durationMs}
    <div class="row">
      {#if s.fiveHour}
        <span>⏱ <span class="w bold">{Math.round(s.fiveHour.pct)}%</span> <span class="d">5h · resets {fmtReset(s.fiveHour.resetsAt)}</span></span>
      {/if}
      {#if s.sevenDay}
        <span class="d sep">│</span>
        <span>📅 <span class="w bold">{Math.round(s.sevenDay.pct)}%</span> <span class="d">7d · resets {fmtReset(s.sevenDay.resetsAt)}</span></span>
      {/if}
      {#if s.costUsd != null}
        <span class="d sep">│</span>
        <span>💰 <span class="m bold">${fmtCost(s.costUsd)}</span></span>
      {/if}
      {#if s.durationMs}
        <span class="d sep">│</span>
        <span>⏺ <span class="t">{fmtMs(s.durationMs)}</span></span>
      {/if}
      {#if s.apiMs}
        <span class="d sep">│</span>
        <span>⚡ <span class="t">{fmtMs(s.apiMs)}</span></span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .sl {
    padding: 0.3em 1ch 0;
    font-size: 13px;
    line-height: 1.5;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    column-gap: 1ch;
    white-space: nowrap;
  }
  .bar {
    column-gap: 0;
  }
  .bar .pct {
    margin-left: 2ch;
    margin-right: 2ch;
  }
  .sep {
    padding: 0 0.5ch;
  }
  .w {
    color: var(--sl-white);
  }
  .d {
    color: var(--sl-dim);
  }
  .r {
    color: var(--sl-red);
  }
  .m {
    color: var(--sl-mauve);
  }
  .t {
    color: var(--sl-teal);
  }
</style>
