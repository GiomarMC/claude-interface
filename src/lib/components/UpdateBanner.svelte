<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { updater } from "$lib/updater.svelte";

  let { busy, oninstall }: { busy: boolean; oninstall: () => void } = $props();

  let current = $state("");
  getVersion().then((v) => (current = v));

  const working = $derived(updater.status === "downloading" || updater.status === "installing");
</script>

{#if (updater.status === "available" && !updater.dismissed) || working}
  <div class="update">
    <span class="claude">✻</span>
    {#if updater.status === "downloading"}
      <span>Descargando v{updater.version}…{updater.progress != null ? ` ${updater.progress}%` : ""}</span>
    {:else if updater.status === "installing"}
      <span>
        Instalando v{updater.version}…
        {#if updater.kind === "rpm" || updater.kind === "deb"}<span class="dim">(introduce tu contraseña en la ventana del sistema)</span>{/if}
      </span>
    {:else}
      <span>Actualización disponible: <span class="bold">v{updater.version}</span> <span class="dim">(tienes v{current})</span></span>
      <span class="actions">
        {#if updater.canInstall}
          <button class="primary" disabled={busy} title={busy ? "Espera a que Claude termine" : ""} onclick={oninstall}>
            {busy ? "Instalar (espera a que Claude termine)" : "Instalar y reiniciar"}
          </button>
        {:else}
          <button class="primary" onclick={() => openUrl(`https://github.com/GiomarMC/claude-interface/releases/tag/v${updater.version}`)}>
            Descargar
          </button>
        {/if}
        <button class="dim" onclick={() => (updater.dismissed = true)}>Más tarde</button>
      </span>
    {/if}
    {#if updater.error}
      <div class="err">{updater.error}</div>
    {/if}
  </div>
{/if}

<style>
  .update {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.3em 1ch;
    border: 1px solid var(--claude);
    border-radius: 8px;
    padding: 0.3em 1.5ch;
    margin: 0 var(--gutter) 0.5em;
  }
  .claude {
    color: var(--claude);
  }
  .actions {
    display: flex;
    gap: 2ch;
    margin-left: auto;
  }
  .primary {
    color: var(--claude);
    font-weight: 700;
  }
  .primary:hover:not(:disabled) {
    text-decoration: underline;
  }
  .primary:disabled {
    color: var(--text-faint);
    cursor: default;
  }
  .err {
    width: 100%;
    color: var(--error);
    font-size: 0.93em;
  }
</style>
