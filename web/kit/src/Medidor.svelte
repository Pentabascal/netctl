<script>
  /**
   * Barra horizontal fina con etiqueta y valor (CPU, memoria, uso…). `porcentaje` 0–100.
   * @type {{ etiqueta: string, valor: string, porcentaje?: number|null, color?: string }}
   */
  let { etiqueta, valor, porcentaje = null, color = 'azul' } = $props();
</script>

<div class="nc-medidor">
  <div class="fila"><span>{etiqueta}</span><strong>{valor}</strong></div>
  <div class="pista"><div class="relleno {color}" style:width="{Math.max(0, Math.min(100, porcentaje ?? 0))}%"></div></div>
</div>

<style>
  .nc-medidor {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .fila {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: 12.5px;
    color: var(--texto-2);
  }
  strong {
    font-weight: 500;
    color: var(--texto);
    font-variant-numeric: tabular-nums;
  }
  .pista {
    height: 4px;
    border-radius: 2px;
    background: var(--hover);
    overflow: hidden;
  }
  .relleno {
    height: 100%;
    border-radius: 2px;
    background: var(--azul);
    transition: width 0.4s;
  }
  .verde {
    background: var(--verde);
  }
  .morado {
    background: var(--morado);
  }
  .ambar {
    background: var(--ambar);
  }
  .rojo {
    background: var(--rojo);
  }
</style>
