<script>
  // Lista de políticas bajo la matriz (UniFi): IPv4/IPv6, buscador, columnas a elegir, las de
  // serie agrupadas con candado, pausar/reanudar, «Crear política» y «Reordenar».
  import { Icono, Interruptor, Enlace, Boton, Aviso } from '@netctl/kit';
  import { aplicado, textoError, textoProto } from './estado.svelte.js';
  import { filtrar, COLUMNAS, mover } from './listaPoliticas.js';

  /** datos: GET /politicas. par: [o, d] | null. */
  let { api, config, datos, par = null, nombreZona, oneditar, oncrear } = $props();

  const c = $derived(config.config);

  const CLAVE = 'netctl.fw.columnas';
  const leerColumnas = () => {
    try {
      const v = JSON.parse(localStorage.getItem(CLAVE) ?? 'null');
      if (Array.isArray(v)) return v;
    } catch {
      /* sin almacenamiento */
    }
    return COLUMNAS.filter((x) => x.porDefecto).map((x) => x.id);
  };
  let visibles = $state(leerColumnas());
  function alternarColumna(id) {
    visibles = visibles.includes(id) ? visibles.filter((x) => x !== id) : COLUMNAS.map((x) => x.id).filter((x) => x === id || visibles.includes(x));
    try {
      localStorage.setItem(CLAVE, JSON.stringify(visibles));
    } catch {
      /* da igual */
    }
  }

  let v4 = $state(true);
  let v6 = $state(true);
  let texto = $state('');
  let buscando = $state(false);
  let eligiendo = $state(false);
  let abiertos = $state({});
  let ordenando = $state(null); // ids mientras se reordena
  let error = $state('');
  let ocupado = $state(false);

  const cols = $derived(COLUMNAS.filter((x) => visibles.includes(x.id)));
  const propias = $derived(filtrar(datos?.propias ?? [], { par, v4, v6, texto }));
  const ordenadas = $derived(ordenando ? ordenando.map((id) => datos.propias.find((r) => r.id === id)).filter(Boolean) : propias);
  const serie = $derived(
    (datos?.de_serie ?? [])
      .map((g) => ({ ...g, parejas: par ? g.parejas.filter((p) => p.origen === par[0] && p.destino === par[1]) : g.parejas }))
      .filter((g) => g.parejas.length && (v4 || v6) && (!texto.trim() || g.nombre.toLowerCase().includes(texto.trim().toLowerCase()))),
  );

  const VERSION = { ambas: 'Ambas', v4: 'IPv4', v6: 'IPv6' };
  const ACCION = { permitir: 'Permitir', bloquear: 'Bloquear', rechazar: 'Rechazar' };
  const proto = (r) => (r.protocolo === 'otro' ? `Nº ${r.numero_protocolo}` : textoProto(r.protocolo));

  function celda(r, id) {
    switch (id) {
      case 'nombre': return r.nombre;
      case 'accion': return ACCION[r.accion] ?? r.accion;
      case 'version_ip': return VERSION[r.version_ip ?? 'ambas'];
      case 'protocolo': return proto(r);
      case 'zona_origen': return r.zona_origen_nombre;
      case 'origen': return r.texto_origen;
      case 'puerto_origen': return r.texto_puertos_origen;
      case 'zona_destino': return r.zona_destino_nombre;
      case 'destino': return r.texto_destino;
      case 'puerto_destino': return r.texto_puertos;
    }
    return '';
  }
  function celdaSerie(g, id) {
    const unica = g.parejas.length === 1 ? g.parejas[0] : null;
    switch (id) {
      case 'nombre': return g.nombre;
      case 'accion': return ACCION[g.accion];
      case 'version_ip': return 'Ambas';
      case 'protocolo': return 'Todos';
      case 'zona_origen': return unica ? nombreZona(unica.origen) : 'Varias';
      case 'zona_destino': return unica ? nombreZona(unica.destino) : 'Varias';
      default: return 'Cualquiera';
    }
  }

  async function pausar(r, activa) {
    error = '';
    try {
      const { id, ...resto } = datos.propias.find((x) => x.id === r.id);
      const limpia = Object.fromEntries(Object.entries(resto).filter(([k]) => !k.startsWith('texto_') && !k.startsWith('zona_') ));
      aplicado(await api.put(`/reglas/${id}`, { regla: { ...limpia, origen_zona: r.origen_zona, destino_zona: r.destino_zona, activa }, revision: c.revision }));
    } catch (e) {
      error = textoError(e);
    }
  }

  async function guardarOrden() {
    ocupado = true;
    error = '';
    try {
      aplicado(await api.post('/reglas/orden', { ids: ordenando, revision: c.revision }));
      ordenando = null;
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }
</script>

<div class="lista">
  <div class="barra">
    <label class="caja"><input type="checkbox" bind:checked={v4} /> IPv4</label>
    <label class="caja"><input type="checkbox" bind:checked={v6} /> IPv6</label>
    <span class="hueco"></span>
    {#if buscando}
      <input class="buscar" type="search" placeholder="Buscar políticas" bind:value={texto} aria-label="Buscar políticas" />
    {/if}
    <button type="button" class="icono" title="Buscar" aria-label="Buscar" onclick={() => { buscando = !buscando; if (!buscando) texto = ''; }}><Icono nombre="buscar" tam={15} /></button>
    <div class="columnas">
      <button type="button" class="icono" title="Elegir columnas" aria-label="Elegir columnas" aria-expanded={eligiendo} onclick={() => (eligiendo = !eligiendo)}><Icono nombre="columnas" tam={15} /></button>
      {#if eligiendo}
        <div class="menu" role="menu">
          {#each COLUMNAS as col (col.id)}
            <label><input type="checkbox" checked={visibles.includes(col.id)} disabled={col.id === 'nombre'} onchange={() => alternarColumna(col.id)} /> {col.titulo}</label>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <div class="desliza">
    <table>
      <thead>
        <tr>
          <th class="estrecha"></th>
          {#each cols as col (col.id)}<th>{col.titulo}</th>{/each}
          <th class="estrecha">{ordenando ? 'Orden' : 'Activa'}</th>
        </tr>
      </thead>
      <tbody>
        {#each ordenadas as r, i (r.id)}
          <tr class:pausada={!r.activa} onclick={() => !ordenando && oneditar(r.id)}>
            <td class="estrecha"></td>
            {#each cols as col (col.id)}<td>{celda(r, col.id)}</td>{/each}
            <td class="estrecha" onclick={(e) => e.stopPropagation()}>
              {#if ordenando}
                <span class="flechas">
                  <button type="button" aria-label="Subir" disabled={i === 0} onclick={() => (ordenando = mover(ordenando, r.id, -1))}><Icono nombre="flecha_arriba" tam={13} /></button>
                  <button type="button" aria-label="Bajar" disabled={i === ordenadas.length - 1} onclick={() => (ordenando = mover(ordenando, r.id, 1))}><Icono nombre="flecha_abajo" tam={13} /></button>
                </span>
              {:else}
                <Interruptor activo={r.activa} etiqueta="Activa" onchange={(v) => pausar(r, v)} />
              {/if}
            </td>
          </tr>
        {/each}
        {#if !ordenando}
          {#each serie as g (g.nombre)}
            <tr class="serie" onclick={() => (abiertos[g.nombre] = !abiertos[g.nombre])}>
              <td class="estrecha"><span class="mas"><Icono nombre={abiertos[g.nombre] ? 'menos' : 'mas'} tam={11} /></span></td>
              {#each cols as col (col.id)}
                <td>
                  {#if col.id === 'nombre'}<span class="candado"><Icono nombre="candado" tam={13} /></span>{/if}
                  {celdaSerie(g, col.id)}{#if col.id === 'nombre' && g.parejas.length > 1}<span class="cuantas"> ({g.parejas.length})</span>{/if}
                </td>
              {/each}
              <td class="estrecha"></td>
            </tr>
            {#if abiertos[g.nombre]}
              {#each g.parejas as p (p.origen + '>' + p.destino)}
                <tr class="pareja">
                  <td class="estrecha"></td>
                  <td colspan={cols.length + 1}>{nombreZona(p.origen)} → {nombreZona(p.destino)}</td>
                </tr>
              {/each}
            {/if}
          {/each}
        {/if}
        {#if !ordenadas.length && !serie.length}
          <tr><td class="vacio" colspan={cols.length + 2}>No hay políticas con estos filtros.</td></tr>
        {/if}
      </tbody>
    </table>
  </div>

  {#if error}<Aviso tipo="error">{error}</Aviso>{/if}

  <div class="acciones">
    {#if ordenando}
      <span class="pista">Las de arriba se miran primero. Sube o baja cada política y guarda.</span>
      <span class="hueco"></span>
      <Boton onclick={() => (ordenando = null)}>Cancelar</Boton>
      <Boton variante="primario" cargando={ocupado} onclick={guardarOrden}>Guardar orden</Boton>
    {:else}
      <Enlace icono="mas" onclick={oncrear}>Crear política</Enlace>
      <Enlace icono="lista" desactivado={(datos?.propias?.length ?? 0) < 2} titulo="Cambiar en qué orden se miran tus políticas" onclick={() => (ordenando = datos.propias.map((r) => r.id))}>Reordenar</Enlace>
    {/if}
  </div>
</div>

<style>
  .lista {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .barra {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .caja {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }
  .caja input {
    accent-color: var(--azul);
  }
  .hueco {
    flex: 1;
  }
  .buscar {
    height: 30px;
    padding: 0 10px;
    width: 220px;
    background: var(--campo);
    border: 0;
    border-radius: var(--radio-pq);
    color: var(--texto);
    font: inherit;
  }
  .icono {
    background: none;
    border: 0;
    color: var(--texto-2);
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    cursor: pointer;
    border-radius: var(--radio-pq);
  }
  .icono:hover {
    color: var(--texto);
    background: var(--hover);
  }
  .columnas {
    position: relative;
  }
  .menu {
    position: absolute;
    right: 0;
    top: 32px;
    z-index: 5;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    background: var(--superficie-2);
    border-radius: var(--radio);
    box-shadow: var(--sombra);
    white-space: nowrap;
  }
  .menu label {
    display: flex;
    gap: 8px;
    align-items: center;
    cursor: pointer;
  }
  .desliza {
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    min-width: 760px;
  }
  th,
  td {
    height: var(--alto-fila);
    padding: 0 8px;
    text-align: left;
    border-bottom: 1px solid var(--borde-suave);
    white-space: nowrap;
  }
  th {
    font-weight: 600;
  }
  tbody tr {
    cursor: pointer;
  }
  tbody tr:hover {
    background: var(--hover);
  }
  .estrecha {
    width: 36px;
    padding: 0 4px;
  }
  .pausada td {
    color: var(--texto-3);
  }
  .serie td {
    color: var(--texto);
  }
  .candado {
    display: inline-flex;
    vertical-align: -2px;
    margin-right: 6px;
    color: var(--texto-2);
  }
  .cuantas {
    color: var(--texto-2);
  }
  .mas {
    width: 16px;
    height: 16px;
    border: 1px solid var(--borde-fuerte);
    border-radius: 3px;
    display: grid;
    place-items: center;
    color: var(--texto-2);
  }
  .pareja td {
    color: var(--texto-2);
    cursor: default;
  }
  .vacio {
    color: var(--texto-2);
    text-align: center;
    cursor: default;
  }
  .flechas {
    display: inline-flex;
    gap: 2px;
  }
  .flechas button {
    background: var(--superficie-2);
    border: 0;
    border-radius: 3px;
    color: var(--texto);
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    cursor: pointer;
  }
  .flechas button:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .acciones {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 4px 8px;
  }
  .pista {
    color: var(--texto-2);
  }
</style>
