<script>
  import Ayuda from './Ayuda.svelte';
  /**
   * Caja de un panel lateral al estilo UniFi: fondo de superficie, título en negrita y su «i».
   * @type {{ titulo?: string, ayuda?: string, ejemplo?: string, children?: any }}
   */
  let { titulo = '', ayuda = '', ejemplo = '', children } = $props();
</script>

<section class="nc-bloque">
  {#if titulo}
    <h3>{titulo}{#if ayuda}<Ayuda texto={ayuda} {ejemplo} titulo="Qué es «{titulo}»" />{/if}</h3>
  {/if}
  {@render children?.()}
</section>

<style>
  .nc-bloque {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    background: var(--superficie);
    border-radius: var(--radio);
  }
  h3 {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    font-weight: 600;
  }
</style>
