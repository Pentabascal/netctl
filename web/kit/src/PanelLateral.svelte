<script>
  import Icono from './Icono.svelte';
  /**
   * Panel que se desliza desde la derecha para ver/editar un elemento (como en UniFi).
   * @type {{ abierto?: boolean, titulo?: string, subtitulo?: string, icono?: string, ancho?: string,
   *   oncerrar?: () => void, cabecera?: any, children?: any, pie?: any }}
   */
  let {
    abierto = $bindable(false),
    titulo = '',
    subtitulo = '',
    icono = '',
    ancho = '420px',
    oncerrar,
    cabecera,
    children,
    pie,
  } = $props();

  function cerrar() {
    abierto = false;
    oncerrar?.();
  }
  function tecla(e) {
    if (abierto && e.key === 'Escape') cerrar();
  }
</script>

<svelte:window onkeydown={tecla} />

{#if abierto}
  <div class="velo" onclick={cerrar} aria-hidden="true"></div>
  <div class="nc-panel" style:--ancho={ancho} role="dialog" aria-modal="true" aria-label={titulo}>
    <header class:centrado={!icono && !subtitulo}>
      {#if icono}<span class="icono"><Icono nombre={icono} tam={18} /></span>{/if}
      <div class="titulos">
        <h2>{titulo}</h2>
        {#if subtitulo}<p>{subtitulo}</p>{/if}
      </div>
      <button class="cerrar" type="button" aria-label="Cerrar" onclick={cerrar}><Icono nombre="cerrar" tam={18} /></button>
    </header>
    {#if cabecera}<div class="sub">{@render cabecera()}</div>{/if}
    <div class="cuerpo">{@render children?.()}</div>
    {#if pie}<footer>{@render pie()}</footer>{/if}
  </div>
{/if}

<style>
  .velo {
    position: fixed;
    inset: 0;
    background: rgba(5, 6, 8, 0.55);
    z-index: 40;
    animation: aparecer 0.15s ease-out;
  }
  .nc-panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(var(--ancho), 100vw);
    z-index: 41;
    display: flex;
    flex-direction: column;
    background: var(--fondo);
    border-left: 1px solid var(--borde-suave);
    box-shadow: var(--sombra);
    animation: deslizar 0.2s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px 18px;
    border-bottom: 1px solid var(--borde-suave);
  }
  .icono {
    width: 36px;
    height: 36px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    background: var(--azul-tinte);
    color: var(--azul);
  }
  .titulos {
    flex: 1;
    min-width: 0;
  }
  .centrado {
    border-bottom: 0;
  }
  .centrado .titulos {
    text-align: center;
    padding-left: 32px;
  }
  .centrado h2 {
    font-size: 13px;
  }
  h2 {
    font-size: 16px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  p {
    color: var(--texto-2);
    font-size: 12.5px;
  }
  .cerrar {
    background: none;
    border: 0;
    color: var(--texto-2);
    width: 32px;
    height: 32px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    cursor: pointer;
  }
  .cerrar:hover {
    background: var(--hover);
    color: var(--texto);
  }
  .sub {
    padding: 0 18px;
  }
  .cuerpo {
    flex: 1;
    overflow-y: auto;
    padding: 4px 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  footer {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    padding: 14px 18px;
    border-top: 1px solid var(--borde-suave);
    background: var(--fondo);
  }
  @keyframes deslizar {
    from {
      transform: translateX(40px);
      opacity: 0;
    }
  }
  @keyframes aparecer {
    from {
      opacity: 0;
    }
  }
</style>
