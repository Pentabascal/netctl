<script>
  import Icono from './Icono.svelte';
  /**
   * variante: primario | secundario | peligro | fantasma
   * @type {{ variante?: string, icono?: string, cargando?: boolean, desactivado?: boolean,
   *   tipo?: 'button'|'submit', tam?: 'normal'|'pequeno', titulo?: string, href?: string,
   *   descargar?: string|boolean, ancho?: boolean, onclick?: (e: MouseEvent) => void, children?: any }}
   */
  let {
    variante = 'secundario',
    icono = '',
    cargando = false,
    desactivado = false,
    tipo = 'button',
    tam = 'normal',
    titulo = '',
    href = '',
    descargar = false,
    ancho = false,
    onclick,
    children,
  } = $props();
</script>

{#if href}
  <a
    class="nc-boton {variante} {tam}"
    class:ancho
    class:solo-icono={!children}
    {href}
    title={titulo || undefined}
    download={descargar || undefined}
    aria-label={titulo || undefined}
  >
    {#if icono}<Icono nombre={icono} tam={tam === 'pequeno' ? 15 : 16} />{/if}
    {#if children}<span>{@render children()}</span>{/if}
  </a>
{:else}
  <button
    class="nc-boton {variante} {tam}"
    class:ancho
    class:solo-icono={!children}
    type={tipo}
    disabled={desactivado || cargando}
    title={titulo || undefined}
    aria-label={titulo || undefined}
    aria-busy={cargando}
    {onclick}
  >
    {#if cargando}
      <span class="giro" aria-hidden="true"></span>
    {:else if icono}
      <Icono nombre={icono} tam={tam === 'pequeno' ? 15 : 16} />
    {/if}
    {#if children}<span>{@render children()}</span>{/if}
  </button>
{/if}

<style>
  .nc-boton {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    height: var(--alto-campo);
    padding: 0 14px;
    border-radius: var(--radio-pq);
    border: 1px solid transparent;
    font: inherit;
    font-size: 13.5px;
    font-weight: 500;
    line-height: 1;
    white-space: nowrap;
    cursor: pointer;
    text-decoration: none;
    transition: background 0.12s, border-color 0.12s, color 0.12s;
    user-select: none;
  }
  .nc-boton:hover {
    text-decoration: none;
  }
  .ancho {
    width: 100%;
  }
  .pequeno {
    height: 28px;
    padding: 0 10px;
    font-size: 12.5px;
  }
  .solo-icono {
    width: var(--alto-campo);
    padding: 0;
  }
  .solo-icono.pequeno {
    width: 28px;
  }
  .primario {
    background: var(--azul);
    color: #fff;
  }
  .primario:hover:not(:disabled) {
    background: var(--azul-hover);
  }
  .secundario {
    background: var(--superficie-2);
    border-color: var(--borde-fuerte);
    color: var(--texto);
  }
  .secundario:hover:not(:disabled) {
    background: var(--hover);
    border-color: var(--texto-2);
  }
  .peligro {
    background: transparent;
    border-color: color-mix(in srgb, var(--rojo) 55%, transparent);
    color: var(--rojo);
  }
  .peligro:hover:not(:disabled) {
    background: var(--rojo-tinte);
  }
  .fantasma {
    background: transparent;
    color: var(--texto-2);
  }
  .fantasma:hover:not(:disabled) {
    background: var(--hover);
    color: var(--texto);
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  button[aria-busy='true'] {
    opacity: 0.8;
    cursor: progress;
  }
  .giro {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid currentColor;
    border-right-color: transparent;
    animation: girar 0.7s linear infinite;
  }
  @keyframes girar {
    to {
      transform: rotate(360deg);
    }
  }
</style>
