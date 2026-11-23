<script>
  import { onMount } from 'svelte';
  import { apiNucleo, crearApi, Barra, Tarjeta, Chip, Medidor, Vacio, Boton, Icono, formatoBytes } from '@netctl/kit';
  import { sesion } from '../sesion.svelte.js';
  import { estadoModulo } from '../estado.js';

  const MEMORIA_MAX = 512 * 1024 * 1024; // MemoryMax=512M de cada unidad de módulo

  let resumenes = $state({}); // id → { datos } | { error }
  let cpu = $state({}); // id → porcentaje
  let previo = {}; // id → { ns, t }

  function medirCpu(modulos) {
    const ahora = performance.now();
    const nuevo = {};
    for (const m of modulos) {
      const p = previo[m.id];
      if (p && m.cpu_ns != null && ahora > p.t) {
        nuevo[m.id] = Math.max(0, ((m.cpu_ns - p.ns) / 1e6 / (ahora - p.t)) * 100);
      } else if (cpu[m.id] != null) {
        nuevo[m.id] = cpu[m.id];
      }
      if (m.cpu_ns != null) previo[m.id] = { ns: m.cpu_ns, t: ahora };
    }
    cpu = nuevo;
  }

  async function refrescar() {
    try {
      const r = await apiNucleo.get('/api/modulos');
      sesion.modulos = r.modulos ?? [];
      medirCpu(sesion.modulos);
    } catch {
      /* se reintenta en el siguiente ciclo */
    }
    await Promise.all(
      sesion.modulos.map(async (m) => {
        try {
          resumenes[m.id] = { datos: await crearApi(`/api/m/${m.id}`).get('/resumen') };
        } catch (e) {
          resumenes[m.id] = { error: e.mensaje };
        }
      }),
    );
  }

  onMount(() => {
    refrescar();
    const t = setInterval(refrescar, 5000);
    return () => clearInterval(t);
  });

  /** Convierte la respuesta de `/resumen` en métricas. Formato preferido:
   *  `{ metricas: [{ etiqueta, valor, detalle?, color? }] }`; si no, se muestran sus campos simples. */
  function metricas(datos) {
    if (!datos || typeof datos !== 'object') return [];
    if (Array.isArray(datos.metricas)) return datos.metricas.slice(0, 4);
    return Object.entries(datos)
      .filter(([, v]) => ['string', 'number', 'boolean'].includes(typeof v))
      .slice(0, 4)
      .map(([k, v]) => ({
        etiqueta: k.replace(/_/g, ' ').replace(/^./, (c) => c.toUpperCase()),
        valor: typeof v === 'boolean' ? (v ? 'Sí' : 'No') : v,
      }));
  }

  function enlace(m) {
    const r = m.menu?.[0]?.ruta?.replace(/^\/+/, '') ?? '';
    return `/modulo/${m.id}${r ? '/' + r : ''}`;
  }

  const enMarcha = $derived(sesion.modulos.filter((m) => m.activo === 'active').length);
</script>

<div class="nc-pagina">
  <Barra titulo="Inicio" subtitulo="Estado del panel y de cada módulo" />

  <div class="metricas">
    <Tarjeta
      etiqueta="Módulos en marcha"
      icono="modulos"
      valor={sesion.modulos.length ? `${enMarcha}` : '0'}
      unidad={sesion.modulos.length ? `de ${sesion.modulos.length}` : ''}
      detalle={sesion.modulos.length === 0 ? 'Ninguno instalado todavía' : enMarcha === sesion.modulos.length ? 'Todo funciona' : 'Hay módulos parados'}
      color={sesion.modulos.length && enMarcha < sesion.modulos.length ? 'ambar' : ''}
    />
    <Tarjeta
      etiqueta="Acceso al panel"
      icono="globo"
      valor={sesion.yo?.alcance === 'internet' ? 'Internet' : 'Solo LAN'}
      detalle={sesion.yo?.alcance === 'internet' ? 'Accesible desde cualquier red' : 'Redes privadas y VPN'}
    />
    <Tarjeta
      etiqueta="Doble factor"
      icono="candado"
      valor={sesion.yo?.totp_activo ? 'Activado' : 'Desactivado'}
      color={sesion.yo?.totp_activo ? 'verde' : ''}
      detalle={sesion.yo?.totp_activo ? 'Se pide un código al entrar' : 'Recomendado antes de abrir a Internet'}
    />
    <Tarjeta etiqueta="Versión del núcleo" icono="servidor" valor={sesion.yo?.version ?? '—'} detalle="netctl-core" />
  </div>

  <section>
    <h2 class="nc-seccion">Módulos</h2>
    {#if sesion.modulos.length === 0}
      <Vacio
        icono="modulos"
        titulo="Aún no hay módulos instalados"
        texto="Cada función del panel (VPN, cortafuegos, red…) es un módulo. Instala el primero desde el catálogo."
      >
        {#snippet acciones()}<Boton variante="primario" icono="mas" href="/modulos">Instalar un módulo</Boton>{/snippet}
      </Vacio>
    {:else}
      <div class="modulos">
        {#each sesion.modulos as m (m.id)}
          {@const est = estadoModulo(m.activo)}
          {@const r = resumenes[m.id]}
          <Tarjeta titulo={m.nombre} subtitulo="v{m.version}" icono={m.icono}>
            {#snippet acciones()}<Chip color={est.color}>{est.texto}</Chip>{/snippet}
            {#if r?.error}
              <p class="sin-resumen"><Icono nombre="aviso" tam={15} />{r.error}</p>
            {:else if r?.datos}
              {@const ms = metricas(r.datos)}
              {#if ms.length}
                <div class="resumen">
                  {#each ms as x, i (i)}
                    <div class="dato">
                      <span class="valor" class:verde={x.color === 'verde'} class:azul={x.color === 'azul'}>{x.valor}</span>
                      <span class="etq">{x.etiqueta}</span>
                      {#if x.detalle}<span class="det">{x.detalle}</span>{/if}
                    </div>
                  {/each}
                </div>
              {/if}
            {:else}
              <div class="resumen cargando"><div class="dato"><span class="valor">·</span><span class="etq">Cargando…</span></div></div>
            {/if}
            <div class="consumo">
              <Medidor
                etiqueta="CPU"
                valor={cpu[m.id] != null ? `${cpu[m.id].toLocaleString('es-ES', { maximumFractionDigits: 1 })} %` : '—'}
                porcentaje={cpu[m.id]}
              />
              <Medidor
                etiqueta="Memoria"
                valor={m.memoria != null ? formatoBytes(m.memoria) : '—'}
                porcentaje={m.memoria != null ? (m.memoria / MEMORIA_MAX) * 100 : null}
                color="morado"
              />
            </div>
            {#snippet pie()}
              <Boton variante="fantasma" tam="pequeno" href={enlace(m)}>Abrir {m.nombre}</Boton>
            {/snippet}
          </Tarjeta>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .metricas {
    display: grid;
    gap: 16px;
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
  .modulos {
    display: grid;
    gap: 16px;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 340px), 1fr));
  }
  .resumen {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(90px, 1fr));
    gap: 12px;
    padding-bottom: 16px;
    margin-bottom: 16px;
    border-bottom: 1px solid var(--borde-suave);
  }
  .dato {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .valor {
    font-size: 20px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.01em;
  }
  .valor.verde {
    color: var(--verde);
  }
  .valor.azul {
    color: var(--azul);
  }
  .etq {
    font-size: 12px;
    color: var(--texto-2);
  }
  .det {
    font-size: 11.5px;
    color: var(--texto-3);
  }
  .cargando {
    opacity: 0.5;
  }
  .sin-resumen {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--ambar);
    font-size: 13px;
    padding-bottom: 16px;
    margin-bottom: 16px;
    border-bottom: 1px solid var(--borde-suave);
  }
  .consumo {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 18px;
  }
  @media (max-width: 1100px) {
    .metricas {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @media (max-width: 420px) {
    .metricas {
      gap: 10px;
    }
  }
</style>
