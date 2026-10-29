<script lang="ts">
  import { highlight } from "$lib/markdown";
  import { relPath, toolArg, toolTitle, type Item } from "$lib/session.svelte";

  let { item, cwd }: { item: Extract<Item, { kind: "tool" }>; cwd: string } = $props();
  let expanded = $state(false);

  const PREVIEW = 4;

  type DiffLine = { kind: "add" | "del" | "ctx"; n: number | null; text: string };

  const diff = $derived.by((): DiffLine[] | null => {
    const { name, input, result } = item;
    if (name !== "Edit" && name !== "MultiEdit") return null;
    const hunks = result?.meta?.structuredPatch;
    const out: DiffLine[] = [];
    if (Array.isArray(hunks) && hunks.length) {
      for (const h of hunks) {
        let oldN = h.oldStart;
        let newN = h.newStart;
        for (const l of h.lines as string[]) {
          const t = l.slice(1);
          if (l[0] === "+") out.push({ kind: "add", n: newN++, text: t });
          else if (l[0] === "-") out.push({ kind: "del", n: oldN++, text: t });
          else {
            out.push({ kind: "ctx", n: newN++, text: t });
            oldN++;
          }
        }
      }
      return out;
    }
    const edits = name === "MultiEdit" ? (input.edits ?? []) : [input];
    for (const e of edits) {
      for (const t of String(e.old_string ?? "").split("\n")) out.push({ kind: "del", n: null, text: t });
      for (const t of String(e.new_string ?? "").split("\n")) out.push({ kind: "add", n: null, text: t });
    }
    return out;
  });

  const summary = $derived.by(() => {
    const { name, input, result } = item;
    if (!result) return null;
    if (result.isError) return null;
    const lines = result.text ? result.text.split("\n").length : 0;
    switch (name) {
      case "Read": {
        const n = result.meta?.file?.numLines ?? Math.max(0, lines - 1);
        return result.images.length ? "Read image" : `Read ${n} lines`;
      }
      case "Edit":
      case "MultiEdit": {
        const add = diff?.filter((d) => d.kind === "add").length ?? 0;
        const del = diff?.filter((d) => d.kind === "del").length ?? 0;
        const parts = [];
        if (add) parts.push(`${add} addition${add === 1 ? "" : "s"}`);
        if (del) parts.push(`${del} removal${del === 1 ? "" : "s"}`);
        return `Updated ${relPath(input.file_path, cwd)}${parts.length ? " with " + parts.join(" and ") : ""}`;
      }
      case "Write": {
        const n = String(input.content ?? "").replace(/\n$/, "").split("\n").length;
        return `Wrote ${n} lines to ${relPath(input.file_path, cwd)}`;
      }
      case "Glob":
        return `Found ${result.meta?.numFiles ?? lines} files`;
      case "Grep":
        return result.meta?.numFiles != null ? `Found ${result.meta.numFiles} files` : `Found ${lines} lines`;
      case "WebSearch":
        return "Did 1 search";
      case "WebFetch":
        return "Received content";
      case "TodoWrite":
        return null;
    }
    return null;
  });

  const outputLines = $derived(item.result?.text ? item.result.text.replace(/\s+$/, "").split("\n") : []);
  const showOutput = $derived(
    !!item.result &&
      (item.result.isError || !["Read", "Edit", "MultiEdit", "Write", "WebFetch", "TodoWrite", "Glob", "Grep"].includes(item.name)),
  );
  const visible = $derived(expanded ? outputLines : outputLines.slice(0, PREVIEW));

  const status = $derived(!item.result ? "pending" : item.result.isError ? "error" : "done");
  const arg = $derived(toolArg(item.name, item.input, cwd));
  const writePreview = $derived(
    item.name === "Write" ? String(item.input.content ?? "").replace(/\n$/, "").split("\n") : [],
  );
</script>

<div class="tool">
  <div class="head">
    <span class="bullet {status}">●</span>
    <span class="title bold">{toolTitle(item.name)}</span>{#if arg}<span class="arg">({arg})</span>{/if}
  </div>

  {#if item.name === "TodoWrite" && Array.isArray(item.input.todos)}
    <div class="body">
      <span class="elbow">⎿</span>
      <div class="todos">
        {#each item.input.todos as t}
          <div class="todo {t.status}">
            {t.status === "completed" ? "☒" : "☐"} {t.status === "in_progress" ? (t.activeForm ?? t.content) : t.content}
          </div>
        {/each}
      </div>
    </div>
  {:else if (item.name === "Task" || item.name === "Agent") && !item.result}
    <div class="body dim">
      <span class="elbow">⎿</span>
      <span>{item.subtasks ? `+${item.subtasks} tool uses` : "Iniciando…"}</span>
    </div>
  {:else if item.result}
    {#if summary}
      <div class="body">
        <span class="elbow">⎿</span>
        <span>{summary}</span>
      </div>
    {/if}

    {#if diff && !item.result.isError}
      <div class="diff">
        {#each expanded ? diff : diff.slice(0, 40) as d}
          <div class="dl {d.kind}">
            <span class="ln">{d.n ?? ""}</span><span class="sign">{d.kind === "add" ? "+" : d.kind === "del" ? "-" : " "}</span><span class="code">{@html highlight(d.text, item.input.file_path)}</span>
          </div>
        {/each}
        {#if diff.length > 40}
          <button class="more" onclick={() => (expanded = !expanded)}>
            {expanded ? "… ocultar" : `… +${diff.length - 40} líneas (clic para expandir)`}
          </button>
        {/if}
      </div>
    {:else if item.name === "Write" && !item.result.isError}
      <div class="diff">
        {#each expanded ? writePreview : writePreview.slice(0, 10) as l, i}
          <div class="dl ctx"><span class="ln">{i + 1}</span><span class="sign"> </span><span class="code">{@html highlight(l, item.input.file_path)}</span></div>
        {/each}
        {#if writePreview.length > 10}
          <button class="more" onclick={() => (expanded = !expanded)}>
            {expanded ? "… ocultar" : `… +${writePreview.length - 10} líneas (clic para expandir)`}
          </button>
        {/if}
      </div>
    {/if}

    {#if showOutput}
      <div class="body" class:err={item.result.isError}>
        <span class="elbow">⎿</span>
        <div class="out">
          {#if !outputLines.length && !item.result.images.length}
            <span class="dim">(Sin salida)</span>
          {/if}
          {#each visible as l}
            <div class="ol">{l || " "}</div>
          {/each}
          {#if outputLines.length > PREVIEW}
            <button class="more" onclick={() => (expanded = !expanded)}>
              {expanded ? "… ocultar" : `… +${outputLines.length - PREVIEW} lines (clic para expandir)`}
            </button>
          {/if}
        </div>
      </div>
    {/if}

    {#each item.result.images as src}
      <div class="body"><span class="elbow">⎿</span><img {src} alt="resultado" /></div>
    {/each}
  {:else}
    <div class="body dim"><span class="elbow">⎿</span><span>Running…</span></div>
  {/if}
</div>

<style>
  .tool {
    margin: 0.9em 0;
  }
  .head {
    display: flex;
    gap: 1ch;
    align-items: baseline;
    overflow-wrap: anywhere;
  }
  .head .arg {
    margin-left: -1ch;
    color: var(--text);
  }
  .bullet {
    flex: none;
  }
  .bullet.done {
    color: var(--success);
  }
  .bullet.error {
    color: var(--error);
  }
  .bullet.pending {
    color: var(--text-dim);
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.3;
    }
  }
  .body {
    display: flex;
    gap: 2ch;
    padding-left: 2ch;
    color: var(--text-dim);
    min-width: 0;
  }
  .body.err {
    color: var(--error);
  }
  .elbow {
    flex: none;
  }
  .out {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .ol {
    white-space: pre-wrap;
  }
  .more {
    color: var(--text-faint);
    font-style: italic;
  }
  .more:hover {
    color: var(--claude);
  }
  .diff {
    margin: 0.2em 0 0 6ch;
    font-size: 0.93em;
    border-radius: 4px;
    overflow-x: auto;
  }
  .dl {
    display: flex;
    white-space: pre;
  }
  .dl .ln {
    width: 5ch;
    text-align: right;
    padding-right: 1ch;
    color: var(--text-faint);
    flex: none;
  }
  .dl .sign {
    width: 2ch;
    flex: none;
  }
  .dl.add {
    background: var(--diff-add);
  }
  .dl.del {
    background: var(--diff-del);
  }
  .dl.add .ln,
  .dl.del .ln {
    color: var(--text);
  }
  .todos .todo.completed {
    text-decoration: line-through;
    color: var(--text-faint);
  }
  .todos .todo.in_progress {
    color: var(--text);
    font-weight: 700;
  }
  img {
    max-width: min(480px, 100%);
    border-radius: 4px;
    margin: 0.3em 0;
  }
</style>
