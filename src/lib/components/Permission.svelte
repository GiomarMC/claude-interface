<script lang="ts">
  import { onMount } from "svelte";
  import { highlight, renderMarkdown } from "$lib/markdown";
  import { relPath, toolTitle, type PermissionRequest, type Session } from "$lib/session.svelte";

  let { s, req }: { s: Session; req: PermissionRequest } = $props();

  type Option = { label: string; run: () => void };

  let selected = $state(0);
  let feedback = $state("");
  let feedbackMode = $state(false);
  let feedbackEl: HTMLInputElement | undefined = $state();
  let root: HTMLDivElement;

  // AskUserQuestion
  const questions: any[] = $derived(req.toolName === "AskUserQuestion" ? (req.input.questions ?? []) : []);
  let qIndex = $state(0);
  let answers: Record<string, string> = {};
  let multi = $state<Set<number>>(new Set());

  const isEdit = $derived(["Edit", "MultiEdit", "Write", "NotebookEdit"].includes(req.toolName));
  const path = $derived(relPath(req.input.file_path ?? req.input.notebook_path ?? "", s.cwd));

  function describeSuggestion(sug: any): string {
    if (sug.type === "setMode" && sug.mode === "acceptEdits") return "Yes, allow all edits during this session";
    if (sug.type === "addRules" || sug.type === "replaceRules") {
      const rules = (sug.rules ?? []).map((r: any) => (r.ruleContent ? `${r.toolName}(${r.ruleContent})` : r.toolName));
      const where = sug.destination === "localSettings" || sug.destination === "projectSettings" ? " in this project" : "";
      return `Yes, and don't ask again for ${rules.join(", ")}${where}`;
    }
    if (sug.type === "addDirectories") return `Yes, and allow access to ${(sug.directories ?? []).join(", ")}`;
    return "Yes, and don't ask again";
  }

  function deny() {
    if (!feedbackMode) {
      feedbackMode = true;
      setTimeout(() => feedbackEl?.focus());
      return;
    }
    const msg = feedback.trim();
    s.answerPermission({
      behavior: "deny",
      message: msg
        ? `El usuario rechazó esta acción y dice: ${msg}`
        : "El usuario rechazó esta acción. Espera sus indicaciones.",
      interrupt: !msg,
    });
  }

  const options: Option[] = $derived.by(() => {
    if (req.toolName === "ExitPlanMode") {
      return [
        {
          label: "Yes, and auto-accept edits",
          run: () =>
            s.answerPermission({
              behavior: "allow",
              updatedPermissions: [{ type: "setMode", mode: "acceptEdits", destination: "session" }],
            }),
        },
        {
          label: "Yes, and manually approve edits",
          run: () =>
            s.answerPermission({
              behavior: "allow",
              updatedPermissions: [{ type: "setMode", mode: "default", destination: "session" }],
            }),
        },
        { label: "No, keep planning", run: deny },
      ];
    }
    if (questions.length) {
      const q = questions[qIndex];
      const opts: Option[] = (q?.options ?? []).map((o: any, i: number) => ({
        label: o.label,
        run: () => (q.multiSelect ? toggleMulti(i) : answer(o.label)),
      }));
      if (q?.multiSelect) opts.push({ label: "Enviar selección", run: () => answer([...multi].map((i) => q.options[i].label).join(", ")) });
      opts.push({ label: "Other (escribir respuesta)", run: () => { feedbackMode = true; setTimeout(() => feedbackEl?.focus()); } });
      return opts;
    }
    const out: Option[] = [{ label: "Yes", run: () => s.answerPermission({ behavior: "allow" }) }];
    for (const sug of req.suggestions.slice(0, 2)) {
      out.push({
        label: describeSuggestion(sug),
        run: () => s.answerPermission({ behavior: "allow", updatedPermissions: [sug] }),
      });
    }
    out.push({ label: "No, and tell Claude what to do differently (esc)", run: deny });
    return out;
  });

  function toggleMulti(i: number) {
    const next = new Set(multi);
    next.has(i) ? next.delete(i) : next.add(i);
    multi = next;
  }

  function answer(value: string) {
    const q = questions[qIndex];
    answers[q.question] = value;
    multi = new Set();
    feedback = "";
    feedbackMode = false;
    if (qIndex < questions.length - 1) {
      qIndex++;
      selected = 0;
      return;
    }
    s.answerPermission({ behavior: "allow", updatedInput: { ...req.input, answers } });
  }

  function submitFeedback() {
    if (questions.length) {
      if (feedback.trim()) answer(feedback.trim());
    } else deny();
  }

  function onKey(e: KeyboardEvent) {
    if (feedbackMode) {
      if (e.key === "Escape") {
        e.preventDefault();
        feedbackMode = false;
        root.focus();
      }
      return;
    }
    if (e.key === "ArrowDown" || e.key === "j") {
      selected = (selected + 1) % options.length;
    } else if (e.key === "ArrowUp" || e.key === "k") {
      selected = (selected - 1 + options.length) % options.length;
    } else if (e.key === "Enter") {
      options[selected]?.run();
    } else if (e.key === "Escape") {
      if (questions.length) s.answerPermission({ behavior: "deny", message: "El usuario no respondió.", interrupt: true });
      else {
        feedbackMode = true;
        deny();
      }
    } else if (/^[1-9]$/.test(e.key)) {
      const i = Number(e.key) - 1;
      if (options[i]) {
        selected = i;
        options[i].run();
      }
    } else return;
    e.preventDefault();
  }

  onMount(() => root.focus());

  const title = $derived.by(() => {
    switch (req.toolName) {
      case "Bash":
        return "Bash command";
      case "Edit":
      case "MultiEdit":
        return "Edit file";
      case "Write":
        return "Create file";
      case "WebFetch":
        return "Fetch";
      case "ExitPlanMode":
        return "Ready to code?";
      case "AskUserQuestion":
        return questions[qIndex]?.header ?? "Pregunta";
    }
    return toolTitle(req.toolName);
  });

  const question = $derived.by(() => {
    if (req.toolName === "ExitPlanMode") return "Would you like to proceed?";
    if (questions.length) return questions[qIndex]?.question;
    if (isEdit) return `Do you want to make this edit to ${path}?`;
    return "Do you want to proceed?";
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="perm" tabindex="0" bind:this={root} onkeydown={onKey} role="dialog" aria-label={title}>
  <div class="title bold">{title}</div>

  <div class="detail">
    {#if req.toolName === "Bash"}
      <div class="cmd">{req.input.command}</div>
      {#if req.input.description}<div class="dim">{req.input.description}</div>{/if}
    {:else if req.toolName === "ExitPlanMode"}
      <div class="plan md">{@html renderMarkdown(req.input.plan ?? "")}</div>
    {:else if req.toolName === "Edit"}
      <div class="dim">{path}</div>
      <div class="diff">
        {#each String(req.input.old_string ?? "").split("\n") as l}<div class="dl del">- {@html highlight(l, path)}</div>{/each}
        {#each String(req.input.new_string ?? "").split("\n") as l}<div class="dl add">+ {@html highlight(l, path)}</div>{/each}
      </div>
    {:else if req.toolName === "Write"}
      <div class="dim">{path}</div>
      <div class="diff">
        {#each String(req.input.content ?? "").split("\n").slice(0, 30) as l}<div class="dl">{@html highlight(l, path)}</div>{/each}
      </div>
    {:else if questions.length}
      {#if questions.length > 1}<div class="dim">Pregunta {qIndex + 1} de {questions.length}</div>{/if}
    {:else}
      <pre class="json">{req.description ? req.description + "\n" : ""}{JSON.stringify(req.input, null, 2)}</pre>
    {/if}
  </div>

  <div class="q">{question}</div>
  <div class="opts">
    {#each options as o, i}
      {@const q = questions[qIndex]}
      <button class="opt" class:sel={i === selected} onmouseenter={() => (selected = i)} onclick={() => o.run()}>
        <span class="cur">{i === selected ? "❯" : " "}</span>
        <span>{i + 1}. {#if q?.multiSelect && i < (q.options?.length ?? 0)}{multi.has(i) ? "[✔] " : "[ ] "}{/if}{o.label}</span>
        {#if q?.options?.[i]?.description}<span class="faint"> — {q.options[i].description}</span>{/if}
      </button>
    {/each}
  </div>

  {#if feedbackMode}
    <div class="fb">
      <span class="dim">&gt;</span>
      <input
        bind:this={feedbackEl}
        bind:value={feedback}
        placeholder={questions.length ? "Escribe tu respuesta…" : "Dile a Claude qué hacer distinto (enter para enviar)"}
        onkeydown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            submitFeedback();
          }
        }}
      />
    </div>
  {/if}
  <div class="hint faint">↑/↓ navegar · enter seleccionar · esc {questions.length ? "cancelar" : "rechazar"}</div>
</div>

<style>
  .perm {
    border: 1px solid var(--permission);
    border-radius: 8px;
    padding: 0.6em 2ch 0.5em;
    margin: 0.8em 0 0.4em;
    outline: none;
  }
  .title {
    color: var(--permission);
  }
  .detail {
    margin: 0.5em 0 0.6em 2ch;
    max-height: 45vh;
    overflow: auto;
  }
  .cmd {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .plan {
    border: 1px solid var(--border-faint);
    border-radius: 6px;
    padding: 0.6em 2ch;
  }
  .diff {
    font-size: 0.93em;
    margin-top: 0.3em;
  }
  .dl {
    white-space: pre-wrap;
  }
  .dl.add {
    background: var(--diff-add);
  }
  .dl.del {
    background: var(--diff-del);
  }
  .json {
    margin: 0;
    white-space: pre-wrap;
    color: var(--text-dim);
    font-family: var(--font);
  }
  .opts {
    display: flex;
    flex-direction: column;
    margin: 0.3em 0;
  }
  .opt {
    display: flex;
    gap: 1ch;
    text-align: left;
    color: var(--text);
  }
  .opt.sel {
    color: var(--permission);
  }
  .cur {
    width: 1ch;
    flex: none;
  }
  .fb {
    display: flex;
    gap: 1ch;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.25em 1ch;
    margin: 0.4em 0;
  }
  .fb input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: var(--text);
    font: inherit;
  }
  .hint {
    font-size: 0.86em;
  }
</style>
