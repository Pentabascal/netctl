<script>
  // WireGuard › Clientes: métricas arriba, tabla en vivo (SSE /vivo) y panel lateral al pulsar.
  import { onMount } from 'svelte';
  import { Barra, Tarjeta, Tabla, Chip, Boton, Aviso, Vacio, Icono, formatoBytes, formatoHace } from '@netctl/kit';
  import PanelCliente from './PanelCliente.svelte';
  import Grafica from './Grafica.svelte';
  import { estadoCliente, formatoTasa, mezclarVivo } from './util.js';

  let { api, navegar, abrir = null } = $props();

  let datos = $state(null);
  let error = $state('');
  let ahora = $state(Date.now() / 1000);
  let filtro = $state('todos');
  let busqueda = $state('');
  let seleccion = $state(null); // índice, 'nuevo' o null
  let bienvenida = $state(false); // recién creado con el asistente
  let ultimoVivo = null;

  async function cargar() {
    try {
      const d = await api.get('/clientes');
      datos = ultimoVivo ? mezclarVivo(d, ultimoVivo) : d;
      error = '';
    } catch (e) {
      error = e.mensaje;
    }
  }

  onMount(() => {
    cargar().then(() => {
      if (abrir != null && datos?.clientes?.[abrir]) {
        seleccion = abrir;
        bienvenida = true;
      }
    });
    const cerrar = api.sse('/vivo', (v) => {
      ultimoVivo = v;
      if (v?.momento) ahora = Math.max(ahora, v.momento);
      if (datos) datos = mezclarVivo(datos, v);
    });
    const reloj = setInterval(() => (ahora = Date.now() / 1000), 5000);
    return () => {
      cerrar();
      clearInterval(reloj);
    };
  });

  const caida = $derived(!!datos?.aviso);

  const filas = $derived(
    (datos?.clientes ?? []).map((c) => {
      const e = estadoCliente(c, caida);
      return {
        ...c,
        id: c.indice,
        est: e,
        orden: e.orden,
        ultimo: c.estado?.ultimo_handshake ?? 0,
        endpoint: c.estado?.endpoint ?? '',
        trafico: (c.estado?.rx ?? 0) + (c.estado?.tx ?? 0),
      };
    }),
  );

  const cuentas = $derived({
    todos: filas.length,
    conectados: filas.filter((f) => f.est.clave === 'conectado').length,
    desconectados: filas.filter((f) => f.est.clave === 'desconectado' || f.est.clave === 'nunca').length,
    desactivados: filas.filter((f) => f.est.clave === 'desactivado').length,
  });

  const filtradas = $derived(
    filas.filter((f) => {
      if (filtro === 'conectados') return f.est.clave === 'conectado';
      if (filtro === 'desconectados') return f.est.clave === 'desconectado' || f.est.clave === 'nunca';
      if (filtro === 'desactivados') return f.est.clave === 'desactivado';
      return true;
    }),
  );

  const filtros = [
    { id: 'todos', titulo: 'Todos' },
    { id: 'conectados', titulo: 'Conectados' },
    { id: 'desconectados', titulo: 'Sin conexión' },
    { id: 'desactivados', titulo: 'Desactivados' },
  ];

  const columnas = [
    { clave: 'nombre', titulo: 'Nombre' },
    { clave: 'orden', titulo: 'Estado', ancho: '168px' },
    { clave: 'ip', titulo: 'IP', ancho: '120px', movil: false },
    { clave: 'ultimo', titulo: 'Último saludo', ancho: '140px', movil: false },
    { clave: 'endpoint', titulo: 'Origen', ancho: '190px', movil: false },
    { clave: 'trafico', titulo: 'Tráfico', ancho: '170px', alinear: 'der', movil: false },
  ];

  const m = $derived(datos?.metricas);
  const historial = $derived(datos?.historial ?? []);
  const clienteSel = $derived(typeof seleccion === 'number' ? datos?.clientes?.[seleccion] : null);

  function icono(nombre) {
    return /m[oó]vil|phone|tel[eé]fono|tablet|ipad|android/i.test(nombre) ? 'movil' : 'usuario';
  }

  function fecha(t) {
    return t ? new Date(t * 1000).toLocaleString('es-ES') : '';
  }

  async function cambiado(nuevoIndice) {
    await cargar();
    if (nuevoIndice != null) seleccion = nuevoIndice;
  }
</script>

<div class="nc-pagina">
  <Barra titulo="Clientes" subtitulo="Dispositivos que se conectan a la VPN · interfaz {datos?.interfaz ?? 'wg0'}" icono="vpn">
    {#snippet acciones()}
      <Boton variante="primario" icono="mas" onclick={() => (seleccion = 'nuevo')}>Nuevo cliente</Boton>
    {/snippet}
  </Barra>

  {#if error}
    <Aviso tipo="error" titulo="No se pudo cargar la lista">
      {error}
      {#snippet acciones()}<Boton tam="pequeno" icono="actualizar" onclick={cargar}>Reintentar</Boton>{/snippet}
    </Aviso>
  {/if}

  {#if datos?.aviso}
    <Aviso tipo="aviso" titulo={datos.aviso}>
      Por eso todos los clientes aparecen sin conexión. Revisa los ajustes del servidor o el estado de
      <code>wg-quick@{datos.interfaz}</code>.
      {#snippet acciones()}<Boton tam="pequeno" onclick={() => navegar('servidor')}>Ver servidor</Boton>{/snippet}
    </Aviso>
  {/if}

  {#if datos}
    <div class="metricas">
      <Tarjeta
        etiqueta="Conectados"
        icono="actividad"
        valor={m.conectados}
        unidad="de {m.total}"
        color={m.conectados ? 'verde' : ''}
        detalle={datos.interfaz_activa ? '' : 'La interfaz no está activa'}
      >
        <div class="reparto" aria-hidden="true">
          <div class="barra-rep">
            {#each [['r-con', cuentas.conectados], ['r-sin', cuentas.desconectados], ['r-off', cuentas.desactivados]] as [clase, n] (clase)}
              {#if n}<span class={clase} style:flex-grow={n}></span>{/if}
            {/each}
          </div>
          <div class="leyenda">
            <span><i class="r-con"></i>{cuentas.conectados} conectado{cuentas.conectados === 1 ? '' : 's'}</span>
            <span><i class="r-sin"></i>{cuentas.desconectados} sin conexión</span>
            {#if cuentas.desactivados}<span><i class="r-off"></i>{cuentas.desactivados} desactivado{cuentas.desactivados === 1 ? '' : 's'}</span>{/if}
          </div>
        </div>
      </Tarjeta>
      <Tarjeta etiqueta="Recibido" icono="flecha_abajo" valor={formatoBytes(m.rx)} detalle="Ahora {formatoTasa(m.tasa_rx)}">
        <Grafica valores={historial.map((h) => h.rx)} color="azul" />
      </Tarjeta>
      <Tarjeta etiqueta="Enviado" icono="flecha_arriba" valor={formatoBytes(m.tx)} detalle="Ahora {formatoTasa(m.tasa_tx)}">
        <Grafica valores={historial.map((h) => h.tx)} color="morado" />
      </Tarjeta>
    </div>

    {#if filas.length === 0}
      <Vacio
        icono="clientes"
        titulo="Aún no hay clientes"
        texto="Crea uno por cada dispositivo que vaya a conectarse: el móvil, el portátil, el de casa…"
      >
        {#snippet acciones()}
          <Boton variante="primario" icono="mas" onclick={() => (seleccion = 'nuevo')}>Nuevo cliente</Boton>
        {/snippet}
      </Vacio>
    {:else}
      <Tabla
        {columnas}
        filas={filtradas}
        buscar
        textoBuscar="Buscar por nombre, IP u origen"
        bind:busqueda
        textoFila={(f) => `${f.nombre} ${f.ip} ${f.endpoint} ${f.acceso}`}
        vacio="Ningún cliente en este filtro"
        seleccionada={typeof seleccion === 'number' ? seleccion : null}
        alPulsar={(f) => (seleccion = f.indice)}
      >
        {#snippet herramientas()}
          <div class="filtros" role="radiogroup" aria-label="Filtrar clientes">
            {#each filtros as f (f.id)}
              <button
                type="button"
                role="radio"
                aria-checked={filtro === f.id}
                class:activo={filtro === f.id}
                onclick={() => (filtro = f.id)}
              >
                {f.titulo}<span class="n">{cuentas[f.id]}</span>
              </button>
            {/each}
          </div>
        {/snippet}
        {#snippet celda(f, c)}
          {#if c.clave === 'nombre'}
            <div class="nombre" class:apagado={!f.activo}>
              <span class="avatar {f.est.clave}"><Icono nombre={icono(f.nombre)} tam={16} /></span>
              <div class="textos">
                <span class="n1">{f.nombre}</span>
                <span class="n2 escritorio">{f.acceso}</span>
                <span class="n2 movil"><span class="mono">{f.ip}</span>{#if !caida} · {formatoHace(f.ultimo, ahora)}{/if}</span>
              </div>
            </div>
          {:else if c.clave === 'orden'}
            <Chip color={f.est.color} punto={f.est.clave !== 'desactivado'}>{f.est.texto}</Chip>
          {:else if c.clave === 'ip'}
            <span class="mono">{f.ip}</span>
          {:else if c.clave === 'ultimo'}
            {#if caida}<span class="tenue">—</span>{:else}<span class:tenue={!f.ultimo} title={fecha(f.ultimo)}>{formatoHace(f.ultimo, ahora)}</span>{/if}
          {:else if c.clave === 'endpoint'}
            {#if f.endpoint}<span class="mono origen">{f.endpoint}</span>{:else}<span class="tenue">—</span>{/if}
          {:else if c.clave === 'trafico'}
            {#if f.trafico}
              <div class="trafico">
                <span><Icono nombre="flecha_abajo" tam={12} trazo={2} />{formatoBytes(f.estado.rx)}</span>
                <span><Icono nombre="flecha_arriba" tam={12} trazo={2} />{formatoBytes(f.estado.tx)}</span>
              </div>
            {:else}
              <span class="tenue">—</span>
            {/if}
          {/if}
        {/snippet}
      </Tabla>
    {/if}
  {:else if !error}
    <div class="metricas">
      {#each [1, 2, 3] as i (i)}<div class="hueco"></div>{/each}
    </div>
  {/if}
</div>

{#if datos && (seleccion === 'nuevo' || clienteSel)}
  <PanelCliente
    {api}
    cliente={clienteSel}
    nuevo={seleccion === 'nuevo'}
    intranet={datos.intranet}
    revision={datos.revision}
    {ahora}
    {navegar}
    oncambiado={cambiado}
    {bienvenida}
    {caida}
    oncerrar={() => ((seleccion = null), (bienvenida = false))}
  />
{/if}

<style>
  .metricas {
    display: grid;
    gap: 16px;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
  .reparto {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 12px;
  }
  .barra-rep {
    display: flex;
    gap: 3px;
    height: 8px;
  }
  .barra-rep span {
    border-radius: 4px;
    flex-basis: 0;
  }
  .leyenda {
    display: flex;
    gap: 6px 14px;
    flex-wrap: wrap;
    font-size: 12px;
    color: var(--texto-2);
  }
  .leyenda span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .leyenda i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  .r-con {
    background: var(--verde);
  }
  .r-sin {
    background: var(--texto-3);
  }
  .r-off {
    background: var(--borde-fuerte);
  }
  .hueco {
    height: 118px;
    border-radius: var(--radio);
    background: var(--superficie);
    border: 1px solid var(--borde-suave);
  }
  .filtros {
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--radio);
    background: var(--campo);
    border: 1px solid var(--borde);
    overflow-x: auto;
  }
  .filtros button {
    display: flex;
    align-items: center;
    gap: 6px;
    border: 0;
    background: none;
    color: var(--texto-2);
    font: inherit;
    font-size: 13px;
    padding: 4px 10px;
    border-radius: var(--radio-pq);
    cursor: pointer;
    white-space: nowrap;
  }
  .filtros button:hover {
    color: var(--texto);
  }
  .filtros button.activo {
    background: var(--superficie-2);
    color: var(--texto);
    box-shadow: 0 0 0 1px var(--borde);
  }
  .n {
    font-size: 11.5px;
    color: var(--texto-3);
    font-variant-numeric: tabular-nums;
  }
  .activo .n {
    color: var(--azul);
  }
  .nombre {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .avatar {
    width: 30px;
    height: 30px;
    flex: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--superficie-2);
    color: var(--texto-2);
    position: relative;
  }
  .avatar.conectado {
    background: var(--verde-tinte);
    color: var(--verde);
  }
  .textos {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .n1 {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .n2 {
    font-size: 12px;
    color: var(--texto-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .apagado .n1 {
    color: var(--texto-2);
  }
  .movil {
    display: none;
  }
  .mono {
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .origen {
    color: var(--texto-2);
  }
  .tenue {
    color: var(--texto-3);
  }
  .trafico {
    display: inline-flex;
    gap: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--texto-2);
  }
  .trafico span {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  @media (max-width: 1000px) {
    .metricas {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .metricas > :global(:first-child) {
      grid-column: 1 / -1;
    }
  }
  @media (max-width: 720px) {
    .metricas {
      gap: 10px;
    }
    .escritorio {
      display: none;
    }
    .movil {
      display: block;
    }
  }
</style>
