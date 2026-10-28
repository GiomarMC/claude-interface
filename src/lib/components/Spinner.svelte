<script lang="ts">
  import { fmtNum } from "$lib/format";
  import { onDestroy } from "svelte";

  let { since, tokens }: { since: number; tokens: number } = $props();

  const FRAMES = ["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];
  const VERBS = [
    "Pondering", "Crafting", "Brewing", "Clauding", "Conjuring", "Noodling",
    "Percolating", "Musing", "Synthesizing", "Tinkering", "Computing", "Cogitating",
  ];
  const verb = VERBS[Math.floor(Math.random() * VERBS.length)];

  let frame = $state(0);
  let now = $state(Date.now());
  const t = setInterval(() => {
    frame = (frame + 1) % FRAMES.length;
    now = Date.now();
  }, 120);
  onDestroy(() => clearInterval(t));

  const secs = $derived(Math.max(0, Math.floor((now - since) / 1000)));
</script>

<div class="spinner">
  <span class="glyph">{FRAMES[frame]}</span>
  <span class="verb">{verb}…</span>
  <span class="dim">(esc to interrupt · {secs}s{tokens ? ` · ↓ ${fmtNum(tokens)} tokens` : ""})</span>
</div>

<style>
  .spinner {
    display: flex;
    gap: 1ch;
    margin: 0.9em 0 0.4em;
    color: var(--claude);
  }
  .glyph {
    width: 1ch;
    flex: none;
  }
  .verb {
    background: linear-gradient(90deg, var(--claude) 0%, var(--claude-shimmer) 50%, var(--claude) 100%);
    background-size: 200% 100%;
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    animation: shimmer 2s linear infinite;
  }
  @keyframes shimmer {
    from {
      background-position: 200% 0;
    }
    to {
      background-position: 0 0;
    }
  }
</style>
