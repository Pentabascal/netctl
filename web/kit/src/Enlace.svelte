<script>
  import Icono from './Icono.svelte';
  /**
   * Acción con aspecto de enlace azul, como «Crear zona» o «Reordenar» bajo las tablas de UniFi.
   * @type {{ icono?: string, desactivado?: boolean, titulo?: string, onclick?: (e: MouseEvent) => void, children?: any }}
   */
  let { icono = '', desactivado = false, titulo = '', onclick, children } = $props();
</script>

<button type="button" class="nc-enlace" disabled={desactivado} title={titulo || undefined} {onclick}>
  {#if icono}<Icono nombre={icono} tam={14} />{/if}{@render children?.()}
</button>

<style>
  .nc-enlace {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 0;
    background: none;
    border: 0;
    color: var(--enlace);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }
  .nc-enlace:hover:not(:disabled) {
    text-decoration: underline;
  }
  .nc-enlace:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
