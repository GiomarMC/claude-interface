<script lang="ts" module>
  export type ClaudeStatus = {
    installed: boolean;
    path?: string | null;
    version?: string | null;
    loggedIn?: boolean | null;
    authMethod?: string | null;
  };
</script>

<script lang="ts">
  // Asistente de primer arranque: instala Claude Code con el instalador
  // oficial e inicia sesión, sin que el usuario abra una terminal.
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { onDestroy, onMount, tick } from "svelte";

  let { status, onready }: { status: ClaudeStatus; onready: (s: ClaudeStatus) => void } = $props();

  type Step = "install" | "login";
  let step = $state<Step>("install");
  /** Último estado conocido (cambia tras instalar). */
  let current = $state<ClaudeStatus | null>(null);
  const version = $derived((current ?? status).version);
  let running = $state(false);
  let output = $state("");
  let error = $state("");
  let code = $state("");
  let selected = $state(0);
  let copied = $state(false);
  let logEl: HTMLPreElement | undefined = $state();
  let root: HTMLDivElement;

  const url = $derived(output.match(/https:\/\/\S+/)?.[0] ?? null);
  const waitingCode = $derived(running && /paste code/i.test(output));

  const unlisten: (() => void)[] = [];
  onMount(async () => {
    step = status.installed ? "login" : "install";
    unlisten.push(
      await listen<string>("setup-output", async (e) => {
        output += e.payload;
        await tick();
        logEl?.scrollTo({ top: logEl.scrollHeight });
      }),
      await listen<number | null>("setup-exit", (e) => finished(e.payload)),
    );
    root.focus();
  });
  onDestroy(() => {
    unlisten.forEach((u) => u());
    if (running) invoke("setup_cancel");
  });

  async function recheck() {
    const s = await invoke<ClaudeStatus>("claude_status");
    current = s;
    if (s.installed && s.loggedIn !== false) return onready(s);
    step = s.installed ? "login" : "install";
    return s;
  }

  async function finished(code: number | null) {
    running = false;
    if (code !== 0) {
      error =
        step === "install"
          ? "La instalación no terminó bien. Revisa la salida de arriba o instálalo a mano."
          : "No se completó el inicio de sesión.";
      return;
    }
    error = "";
    const s = await recheck();
    if (s && step === "install" && !s.installed) {
      error = "El instalador terminó, pero no encuentro `claude`. Reinicia la app o instálalo a mano.";
    }
  }

  async function install() {
    output = "";
    error = "";
    running = true;
    try {
      await invoke("install_claude");
    } catch (e) {
      running = false;
      error = String(e);
    }
  }

  async function login(consoleAccount: boolean) {
    output = "";
    error = "";
    code = "";
    running = true;
    try {
      await invoke("login_claude", { console: consoleAccount });
    } catch (e) {
      running = false;
      error = String(e);
    }
  }

  async function sendCode() {
    if (!code.trim()) return;
    await invoke("setup_input", { text: code });
    code = "";
  }

  function cancel() {
    invoke("setup_cancel");
    running = false;
  }

  const options = $derived(
    step === "install"
      ? [{ label: "Instalar Claude Code", run: install }]
      : [
          { label: "Cuenta de Claude (Pro, Max, Team o Enterprise)", run: () => login(false) },
          { label: "Anthropic Console (pago por uso de la API)", run: () => login(true) },
        ],
  );

  function onKey(e: KeyboardEvent) {
    if (running || (e.target as HTMLElement).tagName === "INPUT") return;
    if (e.key === "ArrowDown") selected = (selected + 1) % options.length;
    else if (e.key === "ArrowUp") selected = (selected - 1 + options.length) % options.length;
    else if (e.key === "Enter") options[selected]?.run();
    else if (/^[1-9]$/.test(e.key) && options[Number(e.key) - 1]) options[Number(e.key) - 1].run();
    else return;
    e.preventDefault();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="setup" bind:this={root} tabindex="0" onkeydown={onKey} role="dialog" aria-label="Configuración inicial">
  <div class="box">
    <div class="legend"><span class="claude">✻</span> Configuración inicial</div>

    <div class="steps">
      <span class:done={step === "login"} class:now={step === "install"}>
        {step === "login" ? "✓" : "1"} Instalar Claude Code
      </span>
      <span class="faint">──</span>
      <span class:now={step === "login"}>2 Iniciar sesión</span>
    </div>

    {#if step === "install"}
      <p>
        Esta app es una interfaz para <span class="bold">Claude Code</span>, que no está instalado en este equipo.
        Se instala con el instalador oficial de Anthropic, solo para tu usuario y sin permisos de administrador.
      </p>
    {:else}
      <p>
        Claude Code está instalado{version ? ` (v${version})` : ""}. Falta iniciar sesión con tu cuenta:
        se abrirá el navegador para autorizarlo.
      </p>
    {/if}

    {#if !running}
      <div class="opts">
        {#each options as o, i}
          <button class="opt" class:sel={i === selected} onmouseenter={() => (selected = i)} onclick={o.run}>
            <span class="cur">{i === selected ? "❯" : " "}</span>
            <span>{i + 1}. {o.label}</span>
          </button>
        {/each}
      </div>
    {/if}

    {#if output}
      <pre class="log" bind:this={logEl}>{output}</pre>
    {/if}

    {#if running}
      <div class="running">
        <span class="claude spin">✻</span>
        {step === "install" ? "Instalando Claude Code…" : "Esperando a que autorices en el navegador…"}
        <button class="link" onclick={cancel}>Cancelar</button>
      </div>
    {/if}

    {#if step === "login" && running && url}
      <div class="url">
        <span class="dim">¿No se abrió el navegador?</span>
        <button class="link" onclick={() => openUrl(url)}>Abrir enlace</button>
        <button
          class="link"
          onclick={async () => {
            await writeText(url);
            copied = true;
            setTimeout(() => (copied = false), 1200);
          }}>{copied ? "✓ copiado" : "Copiar enlace"}</button
        >
      </div>
    {/if}

    {#if waitingCode}
      <div class="code">
        <span class="dim">&gt;</span>
        <input
          bind:value={code}
          placeholder="Si la página te muestra un código, pégalo aquí y pulsa enter"
          onkeydown={(e) => e.key === "Enter" && sendCode()}
        />
      </div>
    {/if}

    {#if error}
      <div class="err">{error}</div>
      <div class="dim manual">
        Instalación manual:
        <button class="link" onclick={() => openUrl("https://docs.claude.com/en/docs/claude-code/setup")}>
          docs.claude.com/en/docs/claude-code/setup
        </button>
      </div>
    {/if}

    <div class="foot faint">
      ↑/↓ navegar · enter seleccionar ·
      <button class="link faint" onclick={() => (step === "install" ? recheck() : onready(current ?? status))}>
        {step === "install" ? "ya lo instalé, comprobar de nuevo" : "continuar sin iniciar sesión"}
      </button>
    </div>
  </div>
</div>

<style>
  .setup {
    display: grid;
    place-items: center;
    min-height: 100%;
    padding: 2em var(--gutter);
    outline: none;
  }
  .box {
    position: relative;
    width: min(760px, 100%);
    border: 1px solid var(--claude);
    border-radius: 8px;
    padding: 1.4em 3ch 1em;
  }
  .legend {
    position: absolute;
    top: -0.75em;
    left: 2ch;
    background: var(--bg);
    padding: 0 1ch;
  }
  .claude {
    color: var(--claude);
  }
  .steps {
    display: flex;
    gap: 1.5ch;
    color: var(--text-faint);
    margin-bottom: 0.8em;
  }
  .steps .now {
    color: var(--text);
    font-weight: 700;
  }
  .steps .done {
    color: var(--success);
  }
  p {
    margin: 0 0 0.9em;
    color: var(--text-dim);
  }
  .opts {
    display: flex;
    flex-direction: column;
    margin: 0.4em 0;
  }
  .opt {
    display: flex;
    gap: 1ch;
    text-align: left;
  }
  .opt.sel {
    color: var(--suggestion);
  }
  .cur {
    width: 1ch;
  }
  .log {
    margin: 0.8em 0 0.4em;
    max-height: 14em;
    overflow: auto;
    background: var(--bg-raised);
    border: 1px solid var(--border-faint);
    border-radius: 6px;
    padding: 0.6em 1.5ch;
    font-family: var(--font);
    font-size: 0.86em;
    color: var(--text-dim);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .running,
  .url {
    display: flex;
    flex-wrap: wrap;
    gap: 1.5ch;
    margin: 0.5em 0;
    align-items: baseline;
  }
  .spin {
    display: inline-block;
    animation: spin 2s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .link {
    color: var(--suggestion);
    text-decoration: underline dotted;
  }
  .link:hover {
    color: var(--claude);
  }
  .code {
    display: flex;
    gap: 1ch;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.25em 1ch;
    margin: 0.5em 0;
  }
  .code input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: var(--text);
    font: inherit;
  }
  .err {
    color: var(--error);
    margin-top: 0.6em;
  }
  .manual {
    margin-top: 0.2em;
  }
  .foot {
    margin-top: 1em;
    font-size: 0.86em;
  }
</style>
