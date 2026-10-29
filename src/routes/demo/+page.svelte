<script lang="ts">
  // Vite sustituye import.meta.env.DEV por una constante, así que en la
  // build de producción la demo (y sus datos de ejemplo) no se incluye.
  const demo = import.meta.env.DEV ? import("./Demo.svelte") : null;
</script>

{#if demo}
  {#await demo then { default: Demo }}
    <Demo />
  {/await}
{:else}
  <p>La demo solo está disponible en desarrollo.</p>
{/if}
