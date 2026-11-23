<script>
  /** @type {{ activo?: boolean, desactivado?: boolean, etiqueta?: string, onchange?: (v: boolean) => void }} */
  let { activo = $bindable(false), desactivado = false, etiqueta = '', onchange } = $props();
  function alternar() {
    if (desactivado) return;
    activo = !activo;
    onchange?.(activo);
  }
</script>

<button
  type="button"
  class="nc-interruptor"
  class:activo
  role="switch"
  aria-checked={activo}
  aria-label={etiqueta || undefined}
  disabled={desactivado}
  onclick={alternar}
>
  <span class="bola"></span>
</button>

<style>
  .nc-interruptor {
    position: relative;
    width: 36px;
    height: 20px;
    flex: none;
    border-radius: 10px;
    border: 0;
    padding: 0;
    background: var(--borde-fuerte);
    cursor: pointer;
    transition: background 0.15s;
  }
  .activo {
    background: var(--azul);
  }
  .bola {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.4);
    transition: transform 0.15s;
  }
  .activo .bola {
    transform: translateX(16px);
  }
  button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
