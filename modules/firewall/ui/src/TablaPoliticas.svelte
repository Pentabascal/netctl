<script>
  // Ajustes › Motor de políticas › Tabla de políticas, como en UniFi: una sola lista con Firewall,
  // NAT y Reenvío de puertos; filtros con contador a la izquierda; columnas a elegir; las de serie
  // agrupadas y con candado; «Reordenar» y «Crear política» debajo. El estado de cada reenvío
  // (funciona, no responde, caduca…) va en la columna Estado.
  import { onMount } from 'svelte';
  import { Icono, Interruptor, Enlace, Boton, Aviso, Chip } from '@netctl/kit';
  import { fw, aplicado, textoError, textoFalta } from './estado.svelte.js';
  import { contadores, aplicarFiltros, filtrosVacios, DIMENSIONES, TIPOS, COLUMNAS_TABLA } from './tabla.js';
  import { mover } from './listaPoliticas.js';
  import FormPolitica from './FormPolitica.svelte';
  import FormNat from './FormNat.svelte';
  import PanelReenvio from './PanelReenvio.svelte';
  import AsistenteReenvio from './AsistenteReenvio.svelte';

  let { api, config } = $props();

  const c = $derived(config.config);
  const nombreZona = (id) => c.zonas.find((z) => z.id === id)?.nombre ?? id;

  let filas = $state([]);
  let reenvios = $state([]);
  let nats = $state([]);
  let error = $state('');
  let f = $state(filtrosVacios());
  let abiertos = $state({});
  let ordenando = $state(null);
  let ocupado = $state(false);
  let panel = $state(null); // { tipo: 'firewall'|'nat'|'reenvio', obj }
  let asistente = $state(false);
  let plegados = $state({});

  async function cargar() {
    try {
      const [t, r, n] = await Promise.all([api.get('/tabla'), api.get('/reenvios'), api.get('/nat')]);
      filas = t;
      reenvios = r.reenvios;
      nats = n.nat;
      error = '';
    } catch (e) {
      error = textoError(e);
    }
  }
  onMount(() => {
    const t = setInterval(cargar, 5000);
    return () => clearInterval(t);
  });
  $effect(() => {
    fw.version;
    cargar();
  });

  // Columnas elegidas (se recuerdan en el navegador).
  const CLAVE = 'netctl.fw.tabla.columnas';
  const leerColumnas = () => {
    try {
      const v = JSON.parse(localStorage.getItem(CLAVE) ?? 'null');
      if (Array.isArray(v)) return v;
    } catch {
      /* sin almacenamiento */
    }
    return COLUMNAS_TABLA.filter((x) => x.porDefecto).map((x) => x.id);
  };
  let visibles = $state(leerColumnas());
  let eligiendo = $state(false);
  function alternarColumna(id) {
    visibles = visibles.includes(id) ? visibles.filter((x) => x !== id) : COLUMNAS_TABLA.map((x) => x.id).filter((x) => x === id || visibles.includes(x));
    try {
      localStorage.setItem(CLAVE, JSON.stringify(visibles));
    } catch {
      /* da igual */
    }
  }
  const cols = $derived(COLUMNAS_TABLA.filter((x) => visibles.includes(x.id)));

  const serie = $derived(filas.filter((x) => x.de_serie));
  const base = $derived(filas.filter((x) => f.serie || !x.de_serie));
  const cuenta = $derived(contadores(base));
  const vistas = $derived(aplicarFiltros(filas, f));
  const propiasFw = $derived(filas.filter((x) => x.tipo === 'firewall' && !x.de_serie));
  const ordenadas = $derived(ordenando ? ordenando.map((id) => propiasFw.find((r) => r.id === id)).filter(Boolean) : vistas);
  const hayFiltros = $derived(f.tipo !== 'todas' || DIMENSIONES.some((d) => f[d.id].length) || f.texto.trim());

  const NOMBRE_TIPO = Object.fromEntries(TIPOS.map((t) => [t.valor, t.texto]));
  const ESTADO = {
    funcionando: ['verde', 'Funcionando'],
    desconectado: ['ambar', 'Equipo desconectado'],
    no_responde: ['ambar', 'No responde en ese puerto'],
    desactivada: ['gris', 'En pausa'],
    comprobando: ['gris', 'Comprobando…'],
  };

  function celda(r, id) {
    if (id === 'tipo') return NOMBRE_TIPO[r.tipo];
    if (r.de_serie && (id === 'zona_origen' || id === 'zona_destino')) {
      const u = r.parejas.length === 1 ? r.parejas[0] : null;
      return u ? nombreZona(id === 'zona_origen' ? u.origen : u.destino) : 'Varias';
    }
    return r[id] ?? '—';
  }

  function alternar(dim, valor) {
    f[dim] = f[dim].includes(valor) ? f[dim].filter((x) => x !== valor) : [...f[dim], valor];
  }

  function abrir(r) {
    if (ordenando) return;
    if (r.de_serie) {
      abiertos[r.id] = !abiertos[r.id];
      return;
    }
    if (r.tipo === 'firewall') {
      const x = c.reglas.find((y) => y.id === r.id);
      if (x) panel = { tipo: 'firewall', obj: x };
    } else if (r.tipo === 'nat') {
      const x = nats.find((y) => y.id === r.id);
      if (x) panel = { tipo: 'nat', obj: x };
    } else {
      const x = reenvios.find((y) => y.id === r.id);
      if (x) panel = { tipo: 'reenvio', obj: x };
    }
  }

  async function activar(r, v) {
    error = '';
    try {
      const rev = { revision: c.revision };
      if (r.tipo === 'firewall') {
        const x = c.reglas.find((y) => y.id === r.id);
        aplicado(await api.put(`/reglas/${r.id}`, { regla: { ...x, activa: v }, ...rev }));
      } else if (r.tipo === 'nat') aplicado(await api.post(`/nat/${r.id}/${v ? 'activar' : 'desactivar'}`, rev));
      else aplicado(await api.post(`/reenvios/${r.id}/${v ? 'activar' : 'desactivar'}`, rev));
    } catch (e) {
      error = textoError(e);
      cargar();
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

  const crear = (tipo = 'firewall') => (panel = { tipo, obj: null });
  const cerrar = () => (panel = null);
</script>

<div class="nc-pagina pagina">
  <aside class="filtros">
    <input class="buscar" type="search" placeholder="Buscar en la tabla" bind:value={f.texto} aria-label="Buscar en la tabla" />
    <Boton icono="mas" onclick={() => crear()}>Crear política</Boton>
    <label class="caja"><input type="checkbox" bind:checked={f.serie} /> Ver las de serie ({serie.length})</label>

    <section>
      <button type="button" class="cab" onclick={() => (plegados.tipo = !plegados.tipo)}>Tipo <Icono nombre={plegados.tipo ? 'chevron_derecha' : 'chevron_abajo'} tam={13} /></button>
      {#if !plegados.tipo}
        <label class="op"><input type="radio" name="tipo" checked={f.tipo === 'todas'} onchange={() => (f.tipo = 'todas')} /> Todas</label>
        {#each TIPOS as t (t.valor)}
          <label class="op" class:cero={!cuenta.tipo[t.valor]}>
            <input type="radio" name="tipo" checked={f.tipo === t.valor} onchange={() => (f.tipo = t.valor)} />
            {t.texto} ({cuenta.tipo[t.valor]})
          </label>
        {/each}
      {/if}
    </section>

    {#each DIMENSIONES as d (d.id)}
      <section>
        <button type="button" class="cab" onclick={() => (plegados[d.id] = !plegados[d.id])}>{d.titulo} <Icono nombre={plegados[d.id] ? 'chevron_derecha' : 'chevron_abajo'} tam={13} /></button>
        {#if !plegados[d.id]}
          {#each Object.entries(cuenta[d.id]) as [valor, n] (valor)}
            <label class="op marca">
              <input type="checkbox" checked={f[d.id].includes(valor)} onchange={() => alternar(d.id, valor)} />
              <span class="txt" title={valor}>{valor}</span>
              <span class="n">({n})</span>
            </label>
          {:else}
            <span class="nada">Nada que filtrar.</span>
          {/each}
        {/if}
      </section>
    {/each}

    <Enlace desactivado={!hayFiltros} onclick={() => (f = { ...filtrosVacios(), serie: f.serie })}>Limpiar filtros</Enlace>
    <div class="columnas">
      <Enlace icono="columnas" onclick={() => (eligiendo = !eligiendo)}>Elegir columnas</Enlace>
      {#if eligiendo}
        <div class="menu" role="menu">
          {#each COLUMNAS_TABLA as col (col.id)}
            <label><input type="checkbox" checked={visibles.includes(col.id)} disabled={col.id === 'nombre'} onchange={() => alternarColumna(col.id)} /> {col.titulo}</label>
          {/each}
        </div>
      {/if}
    </div>
  </aside>

  <div class="principal">
    {#if error}<Aviso tipo="error">{error}</Aviso>{/if}
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
          {#each ordenadas as r, i (r.tipo + r.id)}
            <tr class:pausada={!r.activa} class:serie={r.de_serie} onclick={() => abrir(r)}>
              <td class="estrecha">
                {#if r.de_serie}<span class="mas"><Icono nombre={abiertos[r.id] ? 'menos' : 'mas'} tam={11} /></span>{/if}
              </td>
              {#each cols as col (col.id)}
                <td>
                  {#if col.id === 'estado'}
                    {#if r.tipo === 'reenvio'}
                      {@const e = ESTADO[r.estado] ?? ESTADO.comprobando}
                      <span class="chips">
                        <Chip color={e[0]}>{e[1]}</Chip>
                        {#if r.activa && r.caduca_en != null}<Chip color="azul">Caduca en {textoFalta(r.caduca_en)}</Chip>{/if}
                        {#if r.avisos?.length}<span title={r.avisos.join('\n')}><Chip color="ambar">{r.avisos.length === 1 ? '1 aviso' : `${r.avisos.length} avisos`}</Chip></span>{/if}
                      </span>
                    {:else if !r.activa}
                      <Chip color="gris">En pausa</Chip>
                    {/if}
                  {:else}
                    {#if col.id === 'nombre' && r.de_serie}<span class="candado"><Icono nombre="candado" tam={13} /></span>{/if}
                    {celda(r, col.id)}{#if col.id === 'nombre' && r.de_serie}<span class="cuantas"> ({r.parejas.length})</span>{/if}
                  {/if}
                </td>
              {/each}
              <td class="estrecha" onclick={(e) => e.stopPropagation()}>
                {#if ordenando}
                  <span class="flechas">
                    <button type="button" aria-label="Subir" disabled={i === 0} onclick={() => (ordenando = mover(ordenando, r.id, -1))}><Icono nombre="flecha_arriba" tam={13} /></button>
                    <button type="button" aria-label="Bajar" disabled={i === ordenadas.length - 1} onclick={() => (ordenando = mover(ordenando, r.id, 1))}><Icono nombre="flecha_abajo" tam={13} /></button>
                  </span>
                {:else if !r.de_serie}
                  <Interruptor activo={r.activa} etiqueta="Activa" onchange={(v) => activar(r, v)} />
                {/if}
              </td>
            </tr>
            {#if r.de_serie && abiertos[r.id]}
              {#each r.parejas as p (p.origen + '>' + p.destino)}
                <tr class="pareja">
                  <td class="estrecha"></td>
                  <td colspan={cols.length + 1}>{nombreZona(p.origen)} → {nombreZona(p.destino)}</td>
                </tr>
              {/each}
            {/if}
          {:else}
            <tr><td class="vacio" colspan={cols.length + 2}>{filas.length ? 'No hay políticas con estos filtros.' : 'Aún no hay políticas tuyas. Pulsa «Crear política».'}</td></tr>
          {/each}
        </tbody>
      </table>
    </div>

    <div class="acciones">
      {#if ordenando}
        <span class="pista">Las de arriba se miran primero. Sube o baja cada política de firewall y guarda.</span>
        <span class="hueco"></span>
        <Boton onclick={() => (ordenando = null)}>Cancelar</Boton>
        <Boton variante="primario" cargando={ocupado} onclick={guardarOrden}>Guardar orden</Boton>
      {:else}
        <Enlace icono="lista" desactivado={propiasFw.length < 2} titulo="Cambiar en qué orden se miran tus políticas de firewall" onclick={() => (ordenando = propiasFw.map((r) => r.id))}>Reordenar</Enlace>
        <Enlace icono="mas" onclick={() => crear()}>Crear política</Enlace>
      {/if}
    </div>
  </div>
</div>

{#if panel?.tipo === 'firewall'}
  {#key panel}
    <FormPolitica {api} {config} regla={panel.obj} origen={panel.obj?.origen_zona ?? 'interna'} destino={panel.obj?.destino_zona ?? 'externa'} ontipo={crear} oncerrar={cerrar} />
  {/key}
{:else if panel?.tipo === 'nat'}
  {#key panel}<FormNat {api} {config} nat={panel.obj} ontipo={crear} oncerrar={cerrar} />{/key}
{:else if panel?.tipo === 'reenvio'}
  {#key panel}
    <PanelReenvio {api} {config} reenvio={panel.obj} ontipo={crear} onasistente={() => { panel = null; asistente = true; }} oncerrar={cerrar} />
  {/key}
{/if}
<AsistenteReenvio {api} {config} bind:abierto={asistente} />

<style>
  .pagina {
    display: grid;
    grid-template-columns: 260px minmax(0, 1fr);
    gap: 20px;
    align-items: start;
  }
  .filtros {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-right: 16px;
    border-right: 1px solid var(--borde-suave);
    position: relative;
  }
  .buscar {
    height: 32px;
    padding: 0 10px;
    background: var(--campo);
    border: 0;
    border-radius: var(--radio-pq);
    color: var(--texto);
    font: inherit;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 0 8px;
    border-top: 1px solid var(--borde-suave);
  }
  .cab {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: none;
    border: 0;
    padding: 0 0 2px;
    color: var(--texto);
    font: inherit;
    font-weight: 600;
    font-size: 12.5px;
    cursor: pointer;
  }
  .op,
  .caja {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  .op input,
  .caja input {
    accent-color: var(--azul);
  }
  .op.cero {
    color: var(--texto-3);
  }
  .marca .txt {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .marca .n {
    color: var(--texto-2);
  }
  .nada {
    color: var(--texto-3);
  }
  .columnas {
    position: relative;
  }
  .menu {
    position: absolute;
    left: 0;
    top: 26px;
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
  .principal {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .desliza {
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    min-width: 820px;
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
  .chips {
    display: inline-flex;
    gap: 6px;
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
  .hueco {
    flex: 1;
  }
</style>
