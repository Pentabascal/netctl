<script>
  /**
   * Botón «?» con una explicación más larga, un ejemplo y el valor recomendado (§3b).
   * @type {{ texto?: string, ejemplo?: string, recomendado?: string, titulo?: string }}
   */
  let { texto = '', ejemplo = '', recomendado = '', titulo = 'Más información' } = $props();
  let abierto = $state(false);
  let caja = $state();

  function fuera(e) {
    if (abierto && caja && !caja.contains(e.target)) abierto = false;
  }
  function tecla(e) {
    if (abierto && e.key === 'Escape') abierto = false;
  }
</script>

<svelte:window onclick={fuera} onkeydown={tecla} />

<span class="nc-ayuda" bind:this={caja}>
  <button
    type="button"
    class="boton"
    aria-label={titulo}
    aria-expanded={abierto}
    onclick={(e) => {
      e.preventDefault();
      abierto = !abierto;
    }}>?</button
  >
  {#if abierto}
    <span class="globo" role="note">
      {#if texto}<span class="linea">{texto}</span>{/if}
      {#if ejemplo}<span class="linea"><b>Ejemplo:</b> <code>{ejemplo}</code></span>{/if}
      {#if recomendado}<span class="linea rec"><b>Recomendado:</b> {recomendado}</span>{/if}
    </span>
  {/if}
</span>

<style>
  .nc-ayuda {
    position: relative;
    display: inline-flex;
    vertical-align: middle;
    margin-left: 6px;
  }
  .boton {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 1px solid var(--borde-fuerte);
    background: transparent;
    color: var(--texto-2);
    font: 600 10.5px/1 var(--fuente);
    cursor: pointer;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .boton:hover,
  .boton[aria-expanded='true'] {
    border-color: var(--azul);
    color: var(--azul);
  }
  .globo {
    position: absolute;
    z-index: 60;
    top: 22px;
    left: -8px;
    width: 280px;
    padding: 10px 12px;
    background: var(--superficie-2);
    border: 1px solid var(--borde-fuerte);
    border-radius: var(--radio);
    box-shadow: var(--sombra);
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12.5px;
    font-weight: 400;
    color: var(--texto);
    line-height: 1.5;
    white-space: normal;
  }
  .linea b {
    color: var(--texto-2);
    font-weight: 600;
  }
  .rec b {
    color: var(--verde);
  }
  code {
    font-family: var(--mono);
    font-size: 12px;
    background: var(--codigo);
    padding: 1px 5px;
    border-radius: 4px;
  }
</style>
