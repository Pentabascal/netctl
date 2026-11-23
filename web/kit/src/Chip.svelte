<script>
  /**
   * color: verde | gris | rojo | azul | ambar | morado
   * @type {{ color?: string, punto?: boolean, children?: any }}
   */
  let { color = 'gris', punto = true, children } = $props();
</script>

<span class="nc-chip {color}">
  {#if punto}<span class="punto"></span>{/if}
  {@render children?.()}
</span>

<style>
  .nc-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 22px;
    padding: 0 9px;
    border-radius: 11px;
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    background: var(--hover);
    color: var(--texto-2);
  }
  .punto {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }
  .verde {
    background: var(--verde-tinte);
    color: var(--verde);
  }
  .rojo {
    background: var(--rojo-tinte);
    color: var(--rojo);
  }
  .azul {
    background: var(--azul-tinte);
    color: var(--azul);
  }
  .ambar {
    background: var(--ambar-tinte);
    color: var(--ambar);
  }
  .morado {
    background: var(--morado-tinte);
    color: var(--morado);
  }
</style>
