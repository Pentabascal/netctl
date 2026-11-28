<script>
  // Firewall › Reenvío de puertos: lista agrupada por equipo, con estado en llano, uso y un
  // interruptor por reenvío. «+ Abrir puerto» abre el asistente de 3 pasos.
  import { onMount } from 'svelte';
  import { Barra, Tarjeta, Boton, Chip, Interruptor, Aviso, Vacio, Icono, textoDireccion, formatoHace } from '@netctl/kit';
  import { fw, aplicado, textoError, textoPuertos, textoProto, textoFalta } from './estado.svelte.js';
  import AsistenteReenvio from './AsistenteReenvio.svelte';
  import PanelReenvio from './PanelReenvio.svelte';

  let { api, config, navegar } = $props();

  let datos = $state(null);
  let error = $state('');
  let filtro = $state('todas');
  let busqueda = $state('');
  let asistente = $state(false);
  let equipoInicial = $state(null);
  let abierto = $state(null);
  let ahora = $state(Date.now() / 1000);

  async function cargar() {
    try {
      datos = await api.get('/reenvios');
      error = '';
    } catch (e) {
      error = textoError(e);
    }
  }
  onMount(() => {
    cargar();
    const t = setInterval(() => {
      cargar();
      ahora = Date.now() / 1000;
    }, 5000);
    return () => clearInterval(t);
  });
  $effect(() => {
    fw.version;
    cargar();
  });

  const c = $derived(config.config);
  const nombresGrupos = $derived(Object.fromEntries(c.grupos_ip.map((g) => [g.id, g.nombre])));
  const lista = $derived(datos?.reenvios ?? []);
  const problema = (r) => r.estado === 'desconectado' || r.estado === 'no_responde' || r.avisos?.length > 0;

  const FILTROS = [
    { id: 'todas', titulo: 'Todas', f: () => true },
    { id: 'activas', titulo: 'Activas', f: (r) => r.activo },
    { id: 'desactivadas', titulo: 'Desactivadas', f: (r) => !r.activo },
    { id: 'problemas', titulo: 'Con problemas', f: problema },
    { id: 'temporales', titulo: 'Temporales', f: (r) => r.caduca != null },
  ];

  const sinAcentos = (s) => (s ?? '').normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const visibles = $derived(
    lista.filter(FILTROS.find((x) => x.id === filtro).f).filter((r) => {
      const q = sinAcentos(busqueda.trim());
      if (!q) return true;
      return sinAcentos(`${r.nombre} ${textoDireccion(r.equipo)} ${textoPuertos(r.externo)} ${r.interno ?? ''}`).includes(q);
    }),
  );

  /** Agrupar por equipo (MAC si la tiene; si no, IP). */
  const grupos = $derived.by(() => {
    const g = [];
    for (const r of visibles) {
      const clave = r.equipo.mac ?? r.equipo.red ?? r.equipo.ip;
      let x = g.find((y) => y.clave === clave);
      if (!x) g.push((x = { clave, equipo: r.equipo, reenvios: [] }));
      x.reenvios.push(r);
    }
    return g;
  });

  const ESTADO = {
    funcionando: ['verde', 'Funcionando'],
    desconectado: ['ambar', 'El equipo está desconectado'],
    no_responde: ['ambar', 'El equipo no responde en ese puerto'],
    desactivada: ['gris', 'Desactivada'],
    comprobando: ['gris', 'Comprobando…'],
  };

  async function activar(r, v) {
    try {
      aplicado(await api.post(`/reenvios/${r.id}/${v ? 'activar' : 'desactivar'}`, { revision: c.revision }));
    } catch (e) {
      error = textoError(e);
    }
  }

  function puertosDe(r) {
    const fuera = textoPuertos(r.externo);
    return r.interno != null ? `${fuera} → ${r.interno}` : fuera;
  }
  const desdeTexto = (r) => (r.desde.length ? r.desde.map((d) => textoDireccion(d, nombresGrupos)).join(', ') : 'Cualquier sitio');
  const activos = $derived(lista.filter((r) => r.activo).length);
  const conProblemas = $derived(lista.filter((r) => r.activo && problema(r)).length);
  const temporales = $derived(lista.filter((r) => r.activo && r.caduca != null).length);
</script>

<div class="nc-pagina">
  <Barra titulo="Reenvío de puertos" subtitulo="Deja que desde internet se llegue a un servicio de un equipo de tu red." icono="globo">
    {#snippet acciones()}
      {#if !config.una_interfaz}
        <Boton
          variante="primario"
          icono="mas"
          onclick={() => {
            equipoInicial = null;
            asistente = true;
          }}>Abrir puerto</Boton
        >
      {/if}
    {/snippet}
  </Barra>

  {#if config.una_interfaz}
    <Vacio icono="red" titulo="Esta máquina no es el router de la red" texto="Tiene una sola tarjeta de red, así que no puede reenviar puertos a otros equipos. Para llegar a un equipo de tu red desde internet, abre (redirige) el puerto en el router de tu proveedor hacia ese equipo.">
      {#snippet acciones()}
        <Boton onclick={() => navegar('zonas')}>Abrir un puerto en esta máquina</Boton>
      {/snippet}
    </Vacio>
  {:else}
    <div class="tarjetas">
      <Tarjeta etiqueta="Puertos abiertos" icono="globo" valor={activos} detalle="{lista.length - activos} desactivados" />
      <Tarjeta etiqueta="Con problemas" icono="aviso" valor={conProblemas} color={conProblemas ? 'ambar' : ''} detalle="Equipo apagado o servicio parado" />
      <Tarjeta etiqueta="Temporales" icono="reloj" valor={temporales} detalle="Se desactivan solos al caducar" />
    </div>

    {#if error}<Aviso tipo="error">{error}</Aviso>{/if}

    {#if !lista.length && datos}
      <Vacio icono="globo" titulo="Ningún puerto abierto" texto="Abre un puerto para que se pueda usar desde internet un servicio de tu red: un servidor de juegos, una cámara, un NAS…">
        {#snippet acciones()}<Boton variante="primario" icono="mas" onclick={() => (asistente = true)}>Abrir puerto</Boton>{/snippet}
      </Vacio>
    {:else if datos}
      <div class="herramientas">
        <div class="filtros" role="radiogroup" aria-label="Filtrar">
          {#each FILTROS as x (x.id)}
            <button type="button" role="radio" aria-checked={filtro === x.id} class:activo={filtro === x.id} onclick={() => (filtro = x.id)}>{x.titulo}</button>
          {/each}
        </div>
        <input class="buscar" type="search" placeholder="Buscar por equipo, servicio o puerto" bind:value={busqueda} />
      </div>

      {#each grupos as g (g.clave)}
        {@const r0 = g.reenvios[0]}
        <section class="equipo">
          <header>
            <Icono nombre="red" tam={17} />
            <b>{g.equipo.nombre ?? textoDireccion(g.equipo)}</b>
            <span class="ip">{(g.equipo.ip ?? g.equipo.red ?? '').replace('/32', '')}</span>
            <span class="cuenta">· {g.reenvios.length} {g.reenvios.length === 1 ? 'puerto' : 'puertos'}</span>
            <span class="hueco"></span>
            <Boton
              tam="pequeno"
              variante="fantasma"
              icono="mas"
              onclick={() => {
                equipoInicial = g.equipo;
                asistente = true;
              }}>Otro puerto a este equipo</Boton
            >
          </header>
          {#each g.reenvios as r (r.id)}
            {@const [color, txt] = ESTADO[r.estado] ?? ['gris', r.estado]}
            <div class="fila" class:apagada={!r.activo}>
              <button type="button" class="cuerpo" onclick={() => (abierto = r)}>
                <span class="nombre">{r.nombre}</span>
                <span class="det">
                  <code>{puertosDe(r)}</code> {textoProto(r.protocolo)} · desde {desdeTexto(r).toLowerCase() === 'cualquier sitio' ? 'cualquier sitio' : desdeTexto(r)}
                </span>
                {#each r.avisos ?? [] as a (a)}<span class="aviso"><Icono nombre="aviso" tam={12} /> {a}</span>{/each}
              </button>
              <div class="estado">
                <Chip {color}>{txt}</Chip>
                {#if r.activo && r.caduca_en != null}<span class="caduca"><Icono nombre="reloj" tam={12} /> Caduca en {textoFalta(r.caduca_en)}</span>{/if}
              </div>
              <div class="uso">
                <b>{r.conexiones}</b> conexiones
                <small>{r.ultima ? `última ${formatoHace(r.ultima, ahora)}` : 'sin uso reciente'}</small>
              </div>
              <Interruptor activo={r.activo} etiqueta="Activo" onchange={(v) => activar(r, v)} />
            </div>
          {/each}
        </section>
      {:else}
        <p class="nada">Ningún reenvío en este filtro.</p>
      {/each}
    {/if}
  {/if}
</div>

<AsistenteReenvio {api} {config} bind:abierto={asistente} {equipoInicial} />
{#if abierto}
  <PanelReenvio {api} {config} reenvio={abierto} oncerrar={() => (abierto = null)} />
{/if}

<style>
  .tarjetas {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 12px;
  }
  .herramientas {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: center;
    justify-content: space-between;
  }
  .filtros {
    display: flex;
    gap: 4px;
    padding: 3px;
    background: var(--superficie);
    border: 1px solid var(--borde);
    border-radius: var(--radio);
  }
  .filtros button {
    border: 0;
    background: none;
    color: var(--texto-2);
    font: inherit;
    font-size: 13px;
    padding: 5px 11px;
    border-radius: var(--radio-pq);
    cursor: pointer;
  }
  .filtros button.activo {
    background: var(--superficie-2);
    color: var(--texto);
  }
  .buscar {
    width: 280px;
    height: var(--alto-campo);
    padding: 0 11px;
    background: var(--campo);
    border: 1px solid var(--borde-fuerte);
    border-radius: var(--radio-pq);
    color: var(--texto);
    font: inherit;
  }
  .equipo {
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    background: var(--superficie);
  }
  .equipo header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--borde-suave);
    color: var(--texto-2);
  }
  .equipo header b {
    color: var(--texto);
    font-weight: 600;
  }
  .ip {
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .cuenta {
    font-size: 12.5px;
  }
  .hueco {
    flex: 1;
  }
  .fila {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 240px 150px auto;
    align-items: center;
    gap: 14px;
    padding: 11px 14px;
    border-bottom: 1px solid var(--borde-suave);
  }
  .fila:last-child {
    border-bottom: 0;
  }
  .fila.apagada .cuerpo,
  .fila.apagada .uso {
    opacity: 0.5;
  }
  .cuerpo {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    border: 0;
    background: none;
    padding: 0;
    color: var(--texto);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .nombre {
    font-weight: 500;
  }
  .det {
    color: var(--texto-2);
    font-size: 12.5px;
  }
  code {
    font-family: var(--mono);
    color: var(--texto);
  }
  .aviso {
    display: inline-flex;
    gap: 5px;
    align-items: baseline;
    color: var(--ambar);
    font-size: 12px;
  }
  .estado {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
  }
  .caduca {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    color: var(--ambar);
    font-size: 12px;
  }
  .uso {
    display: flex;
    flex-direction: column;
    font-size: 12.5px;
    color: var(--texto-2);
  }
  .uso b {
    color: var(--texto);
  }
  .uso small {
    color: var(--texto-3);
    font-size: 11.5px;
  }
  .nada {
    color: var(--texto-3);
  }
  @media (max-width: 900px) {
    .fila {
      grid-template-columns: 1fr auto;
    }
    .estado,
    .uso {
      grid-column: 1;
    }
  }
</style>
