<script>
  import Icono from './Icono.svelte';
  /**
   * Tabla estilo UniFi: barra con búsqueda y filtros, cabecera fija, filas pulsables, orden por
   * columna. En móvil las columnas con `movil: false` se ocultan.
   * columnas: [{ clave, titulo, ancho?, alinear?: 'izq'|'der'|'centro', ordenar?: bool, movil?: bool }]
   * @type {{ columnas: any[], filas: any[], clave?: (f: any) => any, buscar?: boolean,
   *   textoBuscar?: string, busqueda?: string, textoFila?: (f: any) => string, vacio?: string,
   *   seleccionada?: any, alPulsar?: (f: any) => void, celda?: any, herramientas?: any,
   *   cargando?: boolean }}
   */
  let {
    columnas,
    filas,
    clave = (f) => f.id,
    buscar = false,
    textoBuscar = 'Buscar',
    busqueda = $bindable(''),
    textoFila,
    vacio = 'No hay nada que mostrar',
    seleccionada = null,
    alPulsar,
    celda,
    herramientas,
    cargando = false,
  } = $props();

  let orden = $state({ clave: null, asc: true });

  function texto(f) {
    if (textoFila) return textoFila(f);
    return columnas.map((c) => f[c.clave] ?? '').join(' ');
  }

  const visibles = $derived.by(() => {
    const q = busqueda.trim().toLocaleLowerCase('es');
    let v = q ? filas.filter((f) => texto(f).toLocaleLowerCase('es').includes(q)) : filas.slice();
    if (orden.clave) {
      const k = orden.clave;
      v.sort((a, b) => {
        const x = a[k];
        const y = b[k];
        const r =
          typeof x === 'number' && typeof y === 'number'
            ? x - y
            : String(x ?? '').localeCompare(String(y ?? ''), 'es', { numeric: true });
        return orden.asc ? r : -r;
      });
    }
    return v;
  });

  function ordenar(c) {
    if (c.ordenar === false) return;
    orden = orden.clave === c.clave ? { clave: c.clave, asc: !orden.asc } : { clave: c.clave, asc: true };
  }
</script>

<div class="nc-tabla">
  {#if buscar || herramientas}
    <div class="barra">
      {#if buscar}
        <label class="buscador">
          <Icono nombre="buscar" tam={16} />
          <input type="search" placeholder={textoBuscar} bind:value={busqueda} aria-label={textoBuscar} />
        </label>
      {/if}
      <div class="nc-separador"></div>
      {@render herramientas?.()}
    </div>
  {/if}
  <div class="marco">
    <table>
      <thead>
        <tr>
          {#each columnas as c (c.clave)}
            <th
              class="al-{c.alinear ?? 'izq'}"
              class:sin-movil={c.movil === false}
              style:width={c.ancho}
              aria-sort={orden.clave === c.clave ? (orden.asc ? 'ascending' : 'descending') : undefined}
            >
              {#if c.ordenar === false}
                {c.titulo}
              {:else}
                <button type="button" onclick={() => ordenar(c)}>
                  {c.titulo}
                  {#if orden.clave === c.clave}
                    <Icono nombre={orden.asc ? 'flecha_arriba' : 'flecha_abajo'} tam={12} trazo={2} />
                  {/if}
                </button>
              {/if}
            </th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each visibles as f (clave(f))}
          <tr
            class:pulsable={!!alPulsar}
            class:sel={seleccionada != null && clave(f) === seleccionada}
            onclick={() => alPulsar?.(f)}
            onkeydown={(e) => e.key === 'Enter' && alPulsar?.(f)}
            tabindex={alPulsar ? 0 : undefined}
          >
            {#each columnas as c (c.clave)}
              <td class="al-{c.alinear ?? 'izq'}" class:sin-movil={c.movil === false}>
                {#if celda}{@render celda(f, c)}{:else}{f[c.clave] ?? '—'}{/if}
              </td>
            {/each}
          </tr>
        {:else}
          <tr class="vacia">
            <td colspan={columnas.length}>{cargando ? 'Cargando…' : busqueda ? 'Nada coincide con la búsqueda' : vacio}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>

<style>
  .nc-tabla {
    display: flex;
    flex-direction: column;
    background: var(--superficie);
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    min-width: 0;
  }
  .barra {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--borde-suave);
    flex-wrap: wrap;
  }
  .buscador {
    display: flex;
    align-items: center;
    gap: 8px;
    height: var(--alto-campo);
    padding: 0 10px;
    width: min(320px, 100%);
    background: var(--campo);
    border: 1px solid var(--borde-fuerte);
    border-radius: var(--radio-pq);
    color: var(--texto-3);
  }
  .buscador:focus-within {
    border-color: var(--azul);
  }
  .buscador input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: none;
    color: var(--texto);
    font: inherit;
  }
  .marco {
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th {
    text-align: left;
    font-size: 12px;
    font-weight: 500;
    color: var(--texto-2);
    padding: 0 14px;
    height: 38px;
    border-bottom: 1px solid var(--borde-suave);
    white-space: nowrap;
    background: var(--superficie);
    position: sticky;
    top: 0;
  }
  th button {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  th button:hover {
    color: var(--texto);
  }
  td {
    padding: 0 14px;
    height: 46px;
    border-bottom: 1px solid var(--borde-suave);
    white-space: nowrap;
  }
  tbody tr:last-child td {
    border-bottom: 0;
  }
  .pulsable {
    cursor: pointer;
  }
  .pulsable:hover td {
    background: var(--hover);
  }
  .sel td {
    background: var(--azul-tinte) !important;
  }
  .al-der {
    text-align: right;
  }
  .al-der button {
    flex-direction: row-reverse;
  }
  .al-centro {
    text-align: center;
  }
  .vacia td {
    text-align: center;
    color: var(--texto-3);
    height: 96px;
  }
  @media (max-width: 720px) {
    .sin-movil {
      display: none;
    }
    th,
    td {
      padding: 0 10px;
    }
  }
</style>
