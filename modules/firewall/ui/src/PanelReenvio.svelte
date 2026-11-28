<script>
  // Panel de un reenvío de puerto con los campos de UniFi, en su orden: nombre, interfaz WAN y su
  // IP, puerto WAN, IP de destino («Elegir equipo»), puerto de destino, protocolo, origen y
  // registro. Además lo nuestro: caducidad, NAT loopback y seguimiento por MAC (al elegir equipo).
  // `reenvio` null = nuevo; con `ontipo` se ve el bloque «Tipo» para cambiar a Firewall o NAT.
  import { untrack } from 'svelte';
  import { PanelLateral, Bloque, Radios, Campo, CampoNumero, SelectorEquipo, Opcion, Boton, Aviso, Icono, Enlace } from '@netctl/kit';
  import { aplicado, textoError, redesDe, textoFalta } from './estado.svelte.js';
  import { deReenvio, aReenvio, formNuevo, puertoUnico, CADUCA, puertosUnifi, dePuertosUnifi } from './reenvio.js';
  import { tiposDisponibles } from './tabla.js';

  let { api, config, reenvio = null, ontipo = null, onasistente = null, oncerrar } = $props();

  const c = $derived(config.config);
  const redes = $derived(redesDe(c));
  const externas = $derived(c.zonas.filter((z) => z.tipo === 'externa').flatMap((z) => Object.entries(c.interfaces).filter(([, x]) => x === z.id).map(([i]) => i)));

  let f = $state(untrack(() => (reenvio ? deReenvio(reenvio) : formNuevo())));
  let p = $state(untrack(() => (reenvio ? puertosUnifi(reenvio) : { wanPuertos: '', destinoPuerto: '' })));
  let wanValido = $state(true);
  let destinoValido = $state(true);
  let comprobado = $state(null);
  let ocupado = $state(false);
  let error = $state('');

  const unico = $derived(puertoUnico(p.wanPuertos));
  const datos = $derived(aReenvio({ ...f, ...dePuertosUnifi(p.wanPuertos, unico != null ? p.destinoPuerto : '') }));

  // IPs de la WAN elegida (o de todas). Una IP guardada que ya no está se sigue viendo, marcada.
  const ipsWan = $derived.by(() => {
    const l = (config.ips_externas ?? []).filter((x) => !f.wan || x.iface === f.wan).map((x) => ({ valor: x.ip, texto: x.ip }));
    if (f.wanIp && !l.some((x) => x.valor === f.wanIp)) l.push({ valor: f.wanIp, texto: `${f.wanIp} (ya no está)` });
    return l;
  });
  $effect(() => {
    f.wan;
    untrack(() => {
      if (f.wanIp && !(config.ips_externas ?? []).some((x) => x.ip === f.wanIp && (!f.wan || x.iface === f.wan))) f.wanIp = '';
    });
  });

  let t;
  $effect(() => {
    const d = JSON.stringify({ ...datos, id: reenvio?.id ?? 0 });
    clearTimeout(t);
    t = setTimeout(async () => {
      try {
        comprobado = await api.post('/reenvios/comprobar', { reenvio: JSON.parse(d) });
      } catch {
        comprobado = null;
      }
    }, 250);
  });

  async function hacer(accion) {
    ocupado = true;
    error = '';
    try {
      const rev = { revision: c.revision };
      if (accion === 'guardar') aplicado(reenvio ? await api.put(`/reenvios/${reenvio.id}`, { reenvio: datos, ...rev }) : await api.post('/reenvios', { reenvio: datos, ...rev }));
      else if (accion === 'borrar') aplicado(await api.del(`/reenvios/${reenvio.id}?revision=${c.revision}`));
      else aplicado(await api.post(`/reenvios/${reenvio.id}/duplicar`, rev));
      oncerrar();
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }

  const caducaOpciones = $derived(
    f.caducaActual ? [{ valor: 'mantener', texto: `Como está (${reenvio?.caduca_en ? 'en ' + textoFalta(reenvio.caduca_en) : 'ya caducó'})` }, ...CADUCA] : CADUCA,
  );
  const errores = $derived(comprobado?.errores ?? []);
  const faltas = $derived(
    [!f.equipo && 'Elige el equipo de destino.', !p.wanPuertos.trim() && 'Escribe el puerto WAN.', (!wanValido || !destinoValido) && 'Revisa los puertos.'].filter(Boolean),
  );
</script>

<PanelLateral abierto titulo={reenvio ? reenvio.nombre : 'Crear política'} subtitulo={reenvio ? 'Reenvío de puerto' : ''} icono={reenvio ? 'globo' : ''} {oncerrar}>
  {#if !reenvio && ontipo}
    <Bloque titulo="Tipo" ayuda="Firewall deja pasar o corta tráfico; NAT cambia direcciones; Reenvío de puertos abre un servicio de tu red a internet.">
      <Radios valor="reenvio" opciones={tiposDisponibles(config.una_interfaz)} onchange={(v) => v !== 'reenvio' && ontipo(v)} />
      {#if onasistente}<Enlace icono="info" onclick={onasistente}>¿Primera vez? Usa el asistente</Enlace>{/if}
    </Bloque>
  {/if}

  {#if reenvio?.como_conectarse?.length}
    <div class="como">
      <Icono nombre="enlace" tam={15} />
      <span>Desde fuera: {#each reenvio.como_conectarse as d, i (d)}{#if i}, {/if}<code>{d}</code>{/each}</span>
    </div>
  {/if}

  <Bloque>
    <Campo etiqueta="Nombre" bind:valor={f.nombre} placeholder="Por ejemplo: Minecraft" ayuda="Para reconocerlo en la lista. Vacío = equipo y puerto." enfocar={!reenvio} />
  </Bloque>

  <Bloque titulo="Entrada desde internet" ayuda="Por dónde llega la conexión de fuera.">
    {#if externas.length > 1}
      <Radios
        etiqueta="Interfaz WAN"
        ayuda="Con varias conexiones a internet, por cuál se abre el puerto. «Todas» vale casi siempre."
        bind:valor={f.wan}
        opciones={[{ valor: '', texto: 'Todas' }, ...externas.map((i) => ({ valor: i, texto: i }))]}
      />
    {/if}
    {#if ipsWan.length > 1 || f.wanIp}
      <Radios
        etiqueta="IP WAN"
        ayuda="Si la conexión tiene varias IPs públicas, a cuál se llama desde fuera. «Todas» vale casi siempre."
        bind:valor={f.wanIp}
        opciones={[{ valor: '', texto: 'Todas' }, ...ipsWan]}
      />
    {/if}
    <CampoNumero
      etiqueta="Puerto WAN"
      bind:valor={p.wanPuertos}
      bind:valido={wanValido}
      puertos
      requerido
      placeholder="Por ejemplo 25565"
      ayuda="El puerto al que se llama desde internet: uno, varios separados por comas o un rango."
      mas={{ ejemplo: '25565  ·  80, 443  ·  27015-27030' }}
    />
  </Bloque>

  <Bloque titulo="Destino en tu red" ayuda="El equipo que da el servicio.">
    <SelectorEquipo
      etiqueta="IP de destino"
      bind:valor={f.equipo}
      soloIpv4
      propias={redes.propias}
      conocidas={redes.conocidas}
      placeholder="Elegir equipo o escribir su IP"
      ayuda="Si lo eliges de la lista, el reenvío le sigue aunque cambie de IP (por su MAC)."
    />
    {#if unico != null}
      <CampoNumero
        etiqueta="Puerto de destino"
        bind:valor={p.destinoPuerto}
        bind:valido={destinoValido}
        placeholder="El mismo: {unico}"
        ayuda="En qué puerto escucha el equipo. Vacío = el mismo que el puerto WAN."
        mas={{ ejemplo: 'WAN 2222 → destino 22 (SSH)' }}
      />
    {/if}
    <Radios
      etiqueta="Protocolo"
      ayuda="TCP lo usan las webs y casi todo; UDP los juegos y la voz. Si no lo sabes, deja «TCP y UDP»."
      bind:valor={f.protocolo}
      opciones={[
        { valor: 'tcp_udp', texto: 'TCP y UDP' },
        { valor: 'tcp', texto: 'TCP' },
        { valor: 'udp', texto: 'UDP' },
      ]}
    />
  </Bloque>

  <Bloque>
    <Radios
      etiqueta="Origen"
      ayuda="Quién puede usar el puerto. «Limitado» lo deja solo para las IPs, redes o rangos que digas: mucho más seguro."
      valor={f.soloAlgunas ? 'limitado' : 'cualquiera'}
      onchange={(v) => (f.soloAlgunas = v === 'limitado')}
      opciones={[
        { valor: 'cualquiera', texto: 'Cualquiera' },
        { valor: 'limitado', texto: 'Limitado' },
      ]}
    />
    {#if f.soloAlgunas}
      <SelectorEquipo bind:valor={f.desde} varios redes soloIpv4 equipos={[]} grupos={c.grupos_ip.map((g) => ({ id: g.id, nombre: g.nombre }))} placeholder="IP, red (198.51.100.0/24) o rango de internet" />
    {/if}
    <Campo etiqueta="Caduca" tipo="select" bind:valor={f.caduca} opciones={caducaOpciones} ayuda="Al caducar se desactiva solo, sin borrarse. Útil para abrir algo un rato." />
    <Opcion
      titulo="Acceso desde dentro con la IP pública"
      texto="Recomendado. Los equipos de tu red también llegan usando la dirección de internet, igual que desde fuera (NAT loopback)."
      bind:activo={f.loopback}
    />
    <Opcion titulo="Registrar (log)" texto="Anota en el registro del sistema cada conexión nueva. Solo para investigar." bind:activo={f.log} />
    <Opcion titulo="Activo" texto="Apágalo para cerrar el puerto sin borrar el reenvío." bind:activo={f.activo} />
  </Bloque>

  {#each errores as e (e)}<Aviso tipo="error">{e}</Aviso>{/each}
  {#each comprobado?.avisos ?? [] as a (a)}<Aviso tipo="aviso">{a}</Aviso>{/each}
  {#if error}<Aviso tipo="error">{error}</Aviso>{/if}

  {#snippet pie()}
    {#if reenvio}
      <Boton variante="peligro" icono="papelera" desactivado={ocupado} onclick={() => hacer('borrar')}>Borrar</Boton>
      <Boton icono="copiar" desactivado={ocupado} onclick={() => hacer('duplicar')} titulo="El mismo servicio a otro equipo (sale desactivado)">Duplicar</Boton>
    {/if}
    <span class="hueco"></span>
    <Boton onclick={oncerrar}>Cancelar</Boton>
    <Boton variante="primario" cargando={ocupado} desactivado={faltas.length > 0 || errores.length > 0} titulo={faltas.join(' ')} onclick={() => hacer('guardar')}>Guardar</Boton>
  {/snippet}
</PanelLateral>

<style>
  .como {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    background: var(--azul-tinte);
    border-radius: var(--radio-pq);
    color: var(--texto-2);
    font-size: 13px;
  }
  code {
    font-family: var(--mono);
    color: var(--texto);
  }
  .hueco {
    flex: 1;
  }
</style>
