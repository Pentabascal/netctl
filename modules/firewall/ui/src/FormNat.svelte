<script>
  // Panel de una regla NAT propia (como «NAT» en el panel de UniFi): Masquerade, SNAT o DNAT, con
  // tarjeta, origen, destino, protocolo y puertos. Van antes que el NAT automático de las zonas.
  // `nat` null = nueva; con `ontipo` se ve el bloque «Tipo» para cambiar a Firewall o Reenvío.
  import { untrack } from 'svelte';
  import { PanelLateral, Bloque, Radios, Campo, CampoNumero, SelectorEquipo, Opcion, Boton, Aviso } from '@netctl/kit';
  import { aplicado, textoError, redesDe } from './estado.svelte.js';
  import { deNat, aNat, nuevaNat, tiposDisponibles } from './tabla.js';

  let { api, config, nat = null, ontipo = null, oncerrar } = $props();

  const c = $derived(config.config);
  const redes = $derived(redesDe(c));
  const tarjetas = $derived(Object.keys(c.interfaces));
  const gruposIp = $derived(c.grupos_ip.map((g) => ({ id: g.id, nombre: g.nombre })));
  const gruposPuertos = $derived(c.grupos_puertos.map((g) => ({ valor: g.id, texto: g.nombre })));

  let f = $state(untrack(() => deNat(nat ?? nuevaNat())));
  let validos = $state({ puertos: true, nuevo: true });
  let ocupado = $state(false);
  let error = $state('');

  const usaPuertos = $derived(['tcp', 'udp', 'tcp_udp'].includes(f.protocolo));
  const entrada = $derived(f.tipo === 'dnat');
  const esIp = (s) => /^(\d{1,3}\.){3}\d{1,3}$/.test(s.trim()) || (s.includes(':') && /^[0-9a-f:.]+$/i.test(s.trim()));

  function modoPuertos(modo) {
    f.puertos = { ...f.puertos, modo };
    if (modo === 'lista' && f.puertos.grupo == null) f.puertos.grupo = c.grupos_puertos[0]?.id ?? null;
  }

  const faltas = $derived.by(() => {
    const l = [];
    if (!f.nombre.trim()) l.push('Ponle un nombre.');
    if (f.tipo !== 'masquerade' && !esIp(f.ip)) l.push('Escribe la IP nueva, por ejemplo 203.0.113.5.');
    if (entrada && f.puertoNuevo.trim() && !validos.nuevo) l.push('Revisa el puerto nuevo.');
    if (entrada && f.puertoNuevo.trim() && !usaPuertos) l.push('Para DNAT con puerto elige TCP o UDP.');
    if (usaPuertos && f.puertos.modo === 'concreto' && !validos.puertos) l.push('Revisa los puertos.');
    if (f.origen.tipo === 'ip' && !f.origen.dir) l.push('Escribe la IP o red de origen.');
    if (f.destino.tipo === 'ip' && !f.destino.dir) l.push('Escribe la IP o red de destino.');
    return l;
  });

  async function hacer(accion) {
    ocupado = true;
    error = '';
    try {
      const rev = { revision: c.revision };
      if (accion === 'guardar') aplicado(nat ? await api.put(`/nat/${nat.id}`, { nat: aNat(f), ...rev }) : await api.post('/nat', { nat: aNat(f), ...rev }));
      else aplicado(await api.del(`/nat/${nat.id}?revision=${c.revision}`));
      oncerrar();
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }

  const DIR = [
    { valor: 'cualquiera', texto: 'Cualquiera' },
    { valor: 'ip', texto: 'IP', ayuda: 'Un equipo, una IP, una red (192.168.1.0/24), un rango o un grupo.' },
  ];
</script>

<PanelLateral abierto titulo={nat ? nat.nombre : 'Crear política'} subtitulo={nat ? 'Regla NAT' : ''} {oncerrar}>
  {#if !nat && ontipo}
    <Bloque titulo="Tipo" ayuda="Firewall deja pasar o corta tráfico; NAT cambia direcciones; Reenvío de puertos abre un servicio de tu red a internet.">
      <Radios valor="nat" opciones={tiposDisponibles(config.una_interfaz)} onchange={(v) => v !== 'nat' && ontipo(v)} />
    </Bloque>
  {/if}

  <Bloque>
    <Campo etiqueta="Nombre" bind:valor={f.nombre} placeholder="Por ejemplo: Oficina sale por la IP 2" ayuda="Para reconocerla en la lista." enfocar={!nat} />
  </Bloque>

  <Bloque titulo="Qué cambia" ayuda="Se aplica antes que el NAT automático de las zonas y que los reenvíos.">
    <Radios
      bind:valor={f.tipo}
      opciones={[
        { valor: 'masquerade', texto: 'Masquerade', ayuda: 'Cambia el origen por la IP de la tarjeta por la que sale. Lo normal para salir a internet.' },
        { valor: 'snat', texto: 'SNAT', ayuda: 'Cambia el origen por una IP fija que elijas, por ejemplo una segunda IP pública.' },
        { valor: 'dnat', texto: 'DNAT', ayuda: 'Cambia el destino por otra IP (y puerto). Para abrir servicios mejor usa Reenvío de puertos.' },
      ]}
    />
    {#if f.tipo !== 'masquerade'}
      <Campo
        etiqueta={f.tipo === 'snat' ? 'IP de origen nueva' : 'IP de destino nueva'}
        bind:valor={f.ip}
        mono
        placeholder={f.tipo === 'snat' ? '203.0.113.5' : '192.168.1.10'}
        ayuda={f.tipo === 'snat' ? 'La IP con la que saldrá el tráfico. Tiene que ser de esta máquina.' : 'El equipo al que se manda el tráfico.'}
        error={f.ip.trim() && !esIp(f.ip) ? 'No es una IP válida.' : ''}
      />
    {/if}
    {#if entrada}
      <CampoNumero
        etiqueta="Puerto nuevo (opcional)"
        bind:valor={f.puertoNuevo}
        bind:valido={validos.nuevo}
        placeholder="El mismo"
        ayuda="Si el equipo escucha en otro puerto. Vacío = el mismo."
        mas={{ ejemplo: '8080' }}
      />
    {/if}
    <Radios
      etiqueta={entrada ? 'Tarjeta de entrada' : 'Tarjeta de salida'}
      ayuda={entrada ? 'Por qué tarjeta llega el tráfico. «Cualquiera» vale para todas.' : 'Por qué tarjeta sale el tráfico. Normalmente la de internet.'}
      bind:valor={f.iface}
      opciones={[{ valor: '', texto: 'Cualquiera' }, ...tarjetas.map((t) => ({ valor: t, texto: t }))]}
    />
  </Bloque>

  <Bloque titulo="Origen" ayuda="De dónde viene el tráfico al que se aplica.">
    <Radios bind:valor={f.origen.tipo} opciones={DIR} />
    {#if f.origen.tipo === 'ip'}
      <SelectorEquipo bind:valor={f.origen.dir} redes grupos={gruposIp} redesLista={redes.lista} conocidas={redes.conocidas} placeholder="IP, red, rango o grupo" />
    {/if}
  </Bloque>

  <Bloque titulo="Destino" ayuda="A dónde iba el tráfico (antes del cambio).">
    <Radios bind:valor={f.destino.tipo} opciones={DIR} />
    {#if f.destino.tipo === 'ip'}
      <SelectorEquipo bind:valor={f.destino.dir} redes grupos={gruposIp} redesLista={redes.lista} conocidas={redes.conocidas} placeholder="IP, red, rango o grupo" />
    {/if}
    <Radios
      etiqueta="Protocolo"
      ayuda="Si no lo sabes, deja «Todos»."
      bind:valor={f.protocolo}
      opciones={[
        { valor: 'cualquiera', texto: 'Todos' },
        { valor: 'tcp_udp', texto: 'TCP y UDP' },
        { valor: 'tcp', texto: 'TCP' },
        { valor: 'udp', texto: 'UDP' },
      ]}
    />
    {#if usaPuertos}
      <Radios
        etiqueta="Puerto de destino"
        ayuda="Solo el tráfico a estos puertos."
        valor={f.puertos.modo}
        onchange={modoPuertos}
        opciones={[
          { valor: 'cualquiera', texto: 'Cualquiera' },
          { valor: 'concreto', texto: 'Concreto', ayuda: 'Uno, varios separados por comas o un rango.' },
          { valor: 'lista', texto: 'Lista', ayuda: 'Un grupo de puertos con nombre (se crean en Grupos).', desactivado: !gruposPuertos.length },
        ]}
      />
      {#if f.puertos.modo === 'concreto'}
        <CampoNumero bind:valor={f.puertos.texto} bind:valido={validos.puertos} puertos requerido placeholder="Por ejemplo 80" mas={{ ejemplo: '80  ·  80, 443  ·  8000-8100' }} />
      {:else if f.puertos.modo === 'lista'}
        <Campo tipo="select" bind:valor={f.puertos.grupo} opciones={gruposPuertos} />
      {/if}
    {/if}
  </Bloque>

  <Bloque>
    <Campo etiqueta="Descripción" tipo="textarea" filas={3} bind:valor={f.descripcion} ayuda="Para acordarte de por qué la pusiste." />
    <Opcion titulo="Activa" texto="Apágala para dejarla en pausa sin borrarla." bind:activo={f.activa} />
  </Bloque>

  {#if error}<Aviso tipo="error">{error}</Aviso>{/if}

  {#snippet pie()}
    {#if nat}<Boton variante="peligro" icono="papelera" desactivado={ocupado} onclick={() => hacer('borrar')}>Borrar</Boton>{/if}
    <span class="hueco"></span>
    <Boton onclick={oncerrar}>Cancelar</Boton>
    <Boton variante="primario" cargando={ocupado} desactivado={faltas.length > 0} titulo={faltas.join(' ')} onclick={() => hacer('guardar')}>Guardar</Boton>
  {/snippet}
</PanelLateral>

<style>
  .hueco {
    flex: 1;
  }
</style>
