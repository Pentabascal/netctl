<script>
  import Icono from './Icono.svelte';
  /**
   * Tarjeta con borde suave. Dos usos:
   *  - contenedor: `titulo`, `subtitulo`, `icono`, `acciones` (snippet) y contenido;
   *  - métrica: `etiqueta` + `valor` (+ `detalle`, `color`), como las de UniFi arriba de cada página.
   * @type {{ titulo?: string, subtitulo?: string, icono?: string, etiqueta?: string,
   *   valor?: string|number, unidad?: string, detalle?: string, color?: string, relleno?: boolean,
   *   pulsable?: boolean, onclick?: () => void, acciones?: any, children?: any, pie?: any }}
   */
  let {
    titulo = '',
    subtitulo = '',
    icono = '',
    etiqueta = '',
    valor = undefined,
    unidad = '',
    detalle = '',
    color = '',
    relleno = true,
    pulsable = false,
    onclick,
    acciones,
    children,
    pie,
  } = $props();
  const esMetrica = $derived(valor !== undefined);
</script>

<svelte:element
  this={pulsable ? 'button' : 'section'}
  class="nc-tarjeta"
  class:metrica={esMetrica}
  class:pulsable
  type={pulsable ? 'button' : undefined}
  role={pulsable ? undefined : 'region'}
  {onclick}
>
  {#if esMetrica}
    <div class="m-cabeza">
      <span class="m-etiqueta">{etiqueta}</span>
      {#if icono}<span class="m-icono {color}"><Icono nombre={icono} tam={16} /></span>{/if}
    </div>
    <div class="m-valor {color}">{valor}{#if unidad}<small>{unidad}</small>{/if}</div>
    {#if detalle}<div class="m-detalle">{detalle}</div>{/if}
    {@render children?.()}
  {:else}
    {#if titulo || acciones}
      <header>
        {#if icono}<span class="t-icono"><Icono nombre={icono} tam={18} /></span>{/if}
        <div class="t-titulos">
          {#if titulo}<h2>{titulo}</h2>{/if}
          {#if subtitulo}<p>{subtitulo}</p>{/if}
        </div>
        {#if acciones}<div class="t-acciones">{@render acciones()}</div>{/if}
      </header>
    {/if}
    {#if children}<div class="cuerpo" class:relleno>{@render children()}</div>{/if}
    {#if pie}<footer>{@render pie()}</footer>{/if}
  {/if}
</svelte:element>

<style>
  .nc-tarjeta {
    display: flex;
    flex-direction: column;
    background: var(--superficie);
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    min-width: 0;
    color: inherit;
    font: inherit;
    text-align: left;
  }
  .pulsable {
    cursor: pointer;
    transition: border-color 0.12s, background 0.12s;
  }
  .pulsable:hover {
    border-color: var(--borde-fuerte);
    background: var(--superficie-2);
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 18px;
    border-bottom: 1px solid var(--borde-suave);
  }
  .t-icono {
    width: 34px;
    height: 34px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    background: var(--azul-tinte);
    color: var(--azul);
    flex: none;
  }
  .t-titulos {
    flex: 1;
    min-width: 0;
  }
  .t-titulos h2 {
    font-size: 15px;
  }
  .t-titulos p {
    color: var(--texto-2);
    font-size: 12.5px;
    margin-top: 1px;
  }
  .t-acciones {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .cuerpo {
    flex: 1;
  }
  .cuerpo.relleno {
    padding: 16px 18px;
  }
  footer {
    padding: 12px 18px;
    border-top: 1px solid var(--borde-suave);
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }

  .metrica {
    padding: 16px 18px;
    gap: 6px;
  }
  .m-cabeza {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .m-etiqueta {
    color: var(--texto-2);
    font-size: 12.5px;
    font-weight: 500;
  }
  .m-icono {
    color: var(--texto-3);
  }
  .m-valor {
    font-size: 26px;
    font-weight: 600;
    letter-spacing: -0.02em;
    line-height: 1.15;
    font-variant-numeric: tabular-nums;
  }
  .m-valor small {
    font-size: 14px;
    font-weight: 500;
    color: var(--texto-2);
    margin-left: 4px;
  }
  .m-detalle {
    color: var(--texto-2);
    font-size: 12.5px;
  }
  @media (max-width: 720px) {
    .m-valor {
      font-size: 22px;
    }
  }
  .verde {
    color: var(--verde);
  }
  .azul {
    color: var(--azul);
  }
  .rojo {
    color: var(--rojo);
  }
  .ambar {
    color: var(--ambar);
  }
  .morado {
    color: var(--morado);
  }
</style>
