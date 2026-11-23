<script>
  import { onMount } from 'svelte';
  import { apiNucleo, Barra, Pestanas, Tarjeta, Chip, Boton, Aviso, Vacio, PanelLateral, Interruptor, Icono } from '@netctl/kit';
  import { sesion, cargarModulos } from '../sesion.svelte.js';
  import { estadoModulo } from '../estado.js';

  const COMANDO_NUCLEO = 'curl -fsSL https://raw.githubusercontent.com/Pentabascal/netctl/master/install.sh | sudo sh';

  let pestana = $state('instalados');
  let catalogo = $state(null);
  let errorCatalogo = $state('');
  let cargandoCatalogo = $state(true);
  let ocupado = $state({}); // id → 'instalar' | 'actualizar' | 'desinstalar'
  let mensaje = $state(null); // { tipo, texto }
  let quitar = $state(null); // módulo a desinstalar
  let borrarDatos = $state(false);
  let copiado = $state(false);

  async function cargarCatalogo() {
    cargandoCatalogo = true;
    errorCatalogo = '';
    try {
      catalogo = await apiNucleo.get('/api/catalogo');
    } catch (e) {
      errorCatalogo = e.mensaje;
    } finally {
      cargandoCatalogo = false;
    }
  }

  onMount(() => {
    cargarCatalogo();
    cargarModulos();
  });

  const porId = $derived(Object.fromEntries((catalogo?.modulos ?? []).map((m) => [m.id, m])));
  const disponibles = $derived((catalogo?.modulos ?? []).filter((m) => !m.instalado));
  const actualizables = $derived(sesion.modulos.filter((m) => porId[m.id]?.actualizable).length);

  async function accion(id, tipo, nombre) {
    ocupado[id] = tipo;
    mensaje = null;
    try {
      if (tipo === 'desinstalar') await apiNucleo.post(`/api/modulos/${id}/desinstalar`, { borrar_datos: borrarDatos });
      else await apiNucleo.post(`/api/modulos/${id}/${tipo}`);
      const hecho = { instalar: 'instalado', actualizar: 'actualizado', desinstalar: 'quitado' }[tipo];
      mensaje = { tipo: 'ok', texto: `${nombre} se ha ${hecho} correctamente.` };
      if (tipo === 'instalar') pestana = 'instalados';
      quitar = null;
      await Promise.all([cargarModulos(), cargarCatalogo()]);
    } catch (e) {
      mensaje = { tipo: 'error', texto: `No se pudo ${tipo} ${nombre}: ${e.mensaje}` };
      quitar = null;
    } finally {
      delete ocupado[id];
    }
  }

  async function copiar() {
    try {
      await navigator.clipboard.writeText(COMANDO_NUCLEO);
      copiado = true;
      setTimeout(() => (copiado = false), 1500);
    } catch {
      /* sin portapapeles (HTTP o permisos) */
    }
  }

  function enlace(m) {
    const r = m.menu?.[0]?.ruta?.replace(/^\/+/, '') ?? '';
    return `/modulo/${m.id}${r ? '/' + r : ''}`;
  }
</script>

<div class="nc-pagina">
  <Barra titulo="Módulos" subtitulo="Instala solo lo que necesites. Cada módulo funciona por separado.">
    {#snippet acciones()}
      <Boton icono="actualizar" cargando={cargandoCatalogo} onclick={cargarCatalogo}>Buscar actualizaciones</Boton>
    {/snippet}
  </Barra>

  {#if catalogo?.nucleo_nuevo}
    <Aviso tipo="info" titulo="Hay una versión nueva del núcleo: {catalogo.nucleo} (tienes {catalogo.nucleo_actual})">
      El núcleo no se actualiza desde aquí. Ejecuta en el servidor:
      <div class="comando">
        <code>{COMANDO_NUCLEO}</code>
        <Boton variante="fantasma" tam="pequeno" icono={copiado ? 'ok' : 'copiar'} titulo="Copiar" onclick={copiar} />
      </div>
    </Aviso>
  {/if}

  {#if mensaje}
    <Aviso tipo={mensaje.tipo} cerrable oncerrar={() => (mensaje = null)}>{mensaje.texto}</Aviso>
  {/if}

  <Pestanas
    bind:activa={pestana}
    pestanas={[
      { id: 'instalados', titulo: 'Instalados', contador: sesion.modulos.length },
      { id: 'disponibles', titulo: 'Disponibles', contador: catalogo ? disponibles.length : undefined },
    ]}
  />

  {#if errorCatalogo}
    <Aviso tipo="error" titulo="No se pudo consultar el catálogo de módulos">
      {errorCatalogo}. Los módulos instalados siguen funcionando.
      {#snippet acciones()}<Boton tam="pequeno" icono="actualizar" onclick={cargarCatalogo}>Reintentar</Boton>{/snippet}
    </Aviso>
  {/if}

  {#if pestana === 'instalados'}
    {#if actualizables}
      <p class="nota"><Icono nombre="flecha_arriba" tam={14} />{actualizables === 1 ? 'Hay 1 módulo con actualización' : `Hay ${actualizables} módulos con actualización`}</p>
    {/if}
    {#if sesion.modulos.length === 0}
      <Vacio icono="modulos" titulo="No hay módulos instalados" texto="Mira la pestaña Disponibles para añadir funciones al panel.">
        {#snippet acciones()}<Boton variante="primario" onclick={() => (pestana = 'disponibles')}>Ver disponibles</Boton>{/snippet}
      </Vacio>
    {:else}
      <div class="lista">
        {#each sesion.modulos as m (m.id)}
          {@const est = estadoModulo(m.activo)}
          {@const c = porId[m.id]}
          <Tarjeta relleno={false}>
            <div class="modulo">
              <span class="icono"><Icono nombre={m.icono} tam={22} /></span>
              <div class="info">
                <div class="nombre">
                  <h3>{m.nombre}</h3>
                  <Chip color={est.color}>{est.texto}</Chip>
                  {#if c?.actualizable}<Chip color="azul" punto={false}>Nueva: v{c.version}</Chip>{/if}
                </div>
                <p>{c?.descripcion ?? `Módulo ${m.id}`}</p>
                <span class="version">Versión {m.version}</span>
              </div>
              <div class="acciones">
                {#if c?.actualizable}
                  <Boton
                    variante="primario"
                    icono="flecha_arriba"
                    cargando={ocupado[m.id] === 'actualizar'}
                    desactivado={!!ocupado[m.id] || !c.compatible}
                    titulo={c.compatible ? '' : `Necesita el núcleo ${c.nucleo_minimo} o superior`}
                    onclick={() => accion(m.id, 'actualizar', m.nombre)}>Actualizar</Boton
                  >
                {/if}
                <Boton href={enlace(m)}>Abrir</Boton>
                <Boton
                  variante="fantasma"
                  icono="papelera"
                  titulo="Quitar {m.nombre}"
                  cargando={ocupado[m.id] === 'desinstalar'}
                  desactivado={!!ocupado[m.id]}
                  onclick={() => ((quitar = m), (borrarDatos = false))}
                />
              </div>
            </div>
          </Tarjeta>
        {/each}
      </div>
    {/if}
  {:else if cargandoCatalogo && !catalogo}
    <div class="lista">
      {#each [0, 1] as i (i)}<div class="hueco"></div>{/each}
    </div>
  {:else if catalogo && disponibles.length === 0}
    <Vacio icono="ok_circulo" titulo="Ya tienes todos los módulos" texto="Cuando se publiquen módulos nuevos aparecerán aquí." />
  {:else if catalogo}
    <div class="lista">
      {#each disponibles as m (m.id)}
        <Tarjeta relleno={false}>
          <div class="modulo">
            <span class="icono"><Icono nombre={m.icono ?? m.id} tam={22} /></span>
            <div class="info">
              <div class="nombre"><h3>{m.nombre}</h3></div>
              <p>{m.descripcion}</p>
              <span class="version">
                Versión {m.version}
                {#if !m.compatible}· <span class="incompatible">necesita el núcleo {m.nucleo_minimo} o superior</span>{/if}
              </span>
            </div>
            <div class="acciones">
              <Boton
                variante="primario"
                icono="descargar"
                cargando={ocupado[m.id] === 'instalar'}
                desactivado={!m.compatible || !!ocupado[m.id]}
                onclick={() => accion(m.id, 'instalar', m.nombre)}>{ocupado[m.id] ? 'Instalando…' : 'Instalar'}</Boton
              >
            </div>
          </div>
        </Tarjeta>
      {/each}
    </div>
  {/if}
</div>

<PanelLateral abierto={!!quitar} titulo="Quitar {quitar?.nombre ?? ''}" icono="papelera" oncerrar={() => (quitar = null)}>
  <p class="texto">
    Se parará el servicio y se borrará el programa. Su entrada desaparecerá del menú. Puedes volver a instalarlo
    cuando quieras desde Disponibles.
  </p>
  <div class="opcion">
    <div>
      <strong>Borrar también sus datos</strong>
      <p>Configuración y estado guardados por el módulo. Si lo dejas apagado, se conservan para una reinstalación.</p>
    </div>
    <Interruptor bind:activo={borrarDatos} etiqueta="Borrar también sus datos" />
  </div>
  {#if borrarDatos}
    <Aviso tipo="aviso">Los datos borrados no se pueden recuperar.</Aviso>
  {/if}
  {#snippet pie()}
    <Boton onclick={() => (quitar = null)}>Cancelar</Boton>
    <Boton
      variante="peligro"
      icono="papelera"
      cargando={quitar && ocupado[quitar.id] === 'desinstalar'}
      onclick={() => accion(quitar.id, 'desinstalar', quitar.nombre)}>Quitar módulo</Boton
    >
  {/snippet}
</PanelLateral>

<style>
  .lista {
    display: grid;
    gap: 12px;
  }
  .modulo {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 16px 18px;
  }
  .icono {
    width: 46px;
    height: 46px;
    flex: none;
    border-radius: 10px;
    display: grid;
    place-items: center;
    background: var(--azul-tinte);
    color: var(--azul);
  }
  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .nombre {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  h3 {
    font-size: 15px;
  }
  .info p {
    color: var(--texto-2);
  }
  .version {
    font-size: 12px;
    color: var(--texto-3);
  }
  .incompatible {
    color: var(--ambar);
  }
  .acciones {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .nota {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--azul);
    font-size: 13px;
    margin: -8px 0 -8px;
  }
  .comando {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
  }
  .comando code {
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    white-space: nowrap;
    padding: 6px 10px;
    color: var(--texto);
  }
  .hueco {
    height: 80px;
    border-radius: var(--radio);
    background: linear-gradient(90deg, var(--superficie) 0%, var(--superficie-2) 50%, var(--superficie) 100%);
    background-size: 200% 100%;
    animation: brillo 1.2s infinite;
    border: 1px solid var(--borde);
  }
  @keyframes brillo {
    to {
      background-position: -200% 0;
    }
  }
  .texto {
    color: var(--texto-2);
  }
  .opcion {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    padding: 14px;
    border: 1px solid var(--borde);
    border-radius: var(--radio);
  }
  .opcion div {
    flex: 1;
  }
  .opcion p {
    color: var(--texto-2);
    font-size: 12.5px;
    margin-top: 3px;
  }
  @media (max-width: 640px) {
    .modulo {
      flex-wrap: wrap;
      align-items: flex-start;
    }
    .acciones {
      width: 100%;
      justify-content: flex-end;
    }
  }
</style>
