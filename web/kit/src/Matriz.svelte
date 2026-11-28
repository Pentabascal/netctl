<script>
  import Icono from './Icono.svelte';
  /**
   * Matriz de zonas como la de UniFi: «Destino» arriba, «Origen» en vertical. Cada casilla dice
   * «Permitir todo» o «Bloquear todo» y «(n)» si tiene reglas propias.
   * Pulsar una casilla la elige (para filtrar la lista de debajo); otra vez, la suelta.
   * El lápiz de la casilla elegida abre su política (`oneditar`).
   * `celda(o, d)` → { politica: 'permitir'|'bloquear', reglas: number, cambiada?: boolean }
   * @type {{ zonas: {id: string, nombre: string}[], celda: (o: string, d: string) => any,
   *   todas?: number, elegida?: string, onfiltrar?: (o: string|null, d: string|null) => void,
   *   oneditar?: (o: string, d: string) => void }}
   */
  let { zonas, celda, todas = 0, elegida = '', onfiltrar, oneditar } = $props();
  let completa = $state(false);

  function pulsar(o, d) {
    if (elegida === `${o}>${d}`) onfiltrar?.(null, null);
    else onfiltrar?.(o, d);
  }
</script>

{#snippet rejilla()}
  <div class="nc-matriz" class:completa>
    <span class="eje-x">Destino</span>
    <span class="eje-y">Origen</span>
    <table role="grid" aria-label="Qué zona puede hablar con cuál">
      <thead>
        <tr>
          <th class="todas">
            <button type="button" class:elegida={!elegida} onclick={() => onfiltrar?.(null, null)}>Todas las políticas ({todas})</button>
          </th>
          {#each zonas as d (d.id)}<th scope="col" class="cab">{d.nombre}</th>{/each}
        </tr>
      </thead>
      <tbody>
        {#each zonas as o (o.id)}
          <tr>
            <th scope="row" class="cab">{o.nombre}</th>
            {#each zonas as d (d.id)}
              {@const c = celda(o.id, d.id)}
              {@const sel = elegida === `${o.id}>${d.id}`}
              <td>
                {#if c}
                  <div class="casilla {c.politica}" class:elegida={sel}>
                    <button
                      type="button"
                      class="pulsar"
                      aria-pressed={sel}
                      aria-label="{o.nombre} hacia {d.nombre}: {c.politica === 'permitir' ? 'permitir todo' : 'bloquear todo'}, {c.reglas} reglas propias"
                      onclick={() => pulsar(o.id, d.id)}
                    >
                      {c.politica === 'permitir' ? 'Permitir todo' : 'Bloquear todo'}{#if c.cambiada}<span class="marca" title="Cambiada respecto a la de serie">•</span>{/if}
                    </button>
                    {#if c.reglas}<span class="num" title="Reglas propias">({c.reglas})</span>{/if}
                    {#if sel && oneditar}
                      <button type="button" class="editar" title="Cambiar la política de {o.nombre} hacia {d.nombre}" aria-label="Cambiar la política" onclick={() => oneditar(o.id, d.id)}>
                        <Icono nombre="editar" tam={13} />
                      </button>
                    {/if}
                  </div>
                {:else}
                  <span class="nada" aria-hidden="true">—</span>
                {/if}
              </td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{/snippet}

<div class="nc-matriz-caja">
  <div class="titulo">
    <button type="button" class="ampliar" title="Ver la matriz a pantalla completa" aria-label="Pantalla completa" onclick={() => (completa = true)}>
      <Icono nombre="ampliar" tam={15} />
    </button>
    <b>Matriz de zonas</b><span class="pista">(pulsa una pareja de zonas para filtrar las políticas de debajo)</span>
  </div>
  {@render rejilla()}
</div>

{#if completa}
  <div class="velo" role="dialog" aria-modal="true" aria-label="Matriz de zonas">
    <div class="grande">
      <div class="titulo">
        <b>Matriz de zonas</b>
        <button type="button" class="ampliar cerrar" aria-label="Cerrar" onclick={() => (completa = false)}><Icono nombre="cerrar" tam={16} /></button>
      </div>
      {@render rejilla()}
    </div>
  </div>
{/if}

<svelte:window onkeydown={(e) => completa && e.key === 'Escape' && (completa = false)} />

<style>
  .nc-matriz-caja {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .titulo {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .pista {
    color: var(--texto-2);
  }
  .ampliar {
    background: none;
    border: 0;
    color: var(--texto-2);
    padding: 2px;
    display: grid;
    place-items: center;
    cursor: pointer;
  }
  .ampliar:hover {
    color: var(--texto);
  }
  .cerrar {
    margin-left: auto;
  }
  .nc-matriz {
    position: relative;
    overflow-x: auto;
    border: 1px solid var(--borde-suave);
    border-radius: var(--radio);
    padding: 30px 16px 16px 36px;
  }
  .eje-x {
    position: absolute;
    top: 8px;
    left: 50%;
    transform: translateX(-50%);
    color: var(--texto-2);
  }
  .eje-y {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translate(-50%, -50%) rotate(-90deg) translateY(50%);
    color: var(--texto-2);
  }
  table {
    border-collapse: separate;
    border-spacing: 2px;
    width: 100%;
    min-width: 640px;
    table-layout: fixed;
  }
  th,
  td {
    padding: 0;
    height: 30px;
    text-align: center;
    font-weight: 400;
  }
  .cab {
    background: var(--superficie);
    border-radius: 3px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 0 6px;
  }
  .todas button {
    width: 100%;
    height: 30px;
    border: 0;
    border-radius: 3px;
    background: var(--superficie);
    color: var(--enlace);
    font: inherit;
    cursor: pointer;
  }
  .todas button.elegida {
    background: var(--azul-tinte);
  }
  .casilla {
    position: relative;
    height: 30px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    border-radius: 3px;
    border: 1px solid transparent;
  }
  .casilla.permitir {
    background: var(--verde-tinte);
    border-color: var(--verde-tinte);
  }
  .casilla.bloquear {
    background: var(--rojo-tinte);
    border-color: var(--rojo-tinte);
  }
  .casilla.elegida {
    border-color: var(--azul);
  }
  .pulsar {
    flex: 1;
    height: 100%;
    background: none;
    border: 0;
    color: var(--texto);
    font: inherit;
    cursor: pointer;
    white-space: nowrap;
  }
  .casilla:hover {
    filter: brightness(1.25);
  }
  .num {
    position: absolute;
    right: 8px;
    color: var(--enlace);
  }
  .editar {
    position: absolute;
    left: 4px;
    background: var(--fondo);
    border: 0;
    border-radius: 3px;
    color: var(--texto);
    width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    cursor: pointer;
  }
  .marca {
    color: var(--azul);
    margin-left: 3px;
  }
  .nada {
    color: var(--texto-3);
  }
  .velo {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: rgba(5, 6, 8, 0.7);
    display: grid;
    place-items: center;
    padding: 24px;
  }
  .grande {
    width: min(1400px, 100%);
    max-height: 100%;
    overflow: auto;
    background: var(--fondo);
    border-radius: var(--radio);
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
</style>
