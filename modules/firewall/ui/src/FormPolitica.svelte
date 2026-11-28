<script>
  // Crear o editar una política, como el panel «Create Policy» de UniFi: un bloque por parte
  // (Nombre, Origen, Acción, Destino, Opciones), opciones como radios en línea y cada campo con
  // su «?» (§3b).
  import { untrack } from 'svelte';
  import { PanelLateral, Bloque, Radios, Campo, CampoNumero, SelectorEquipo, Opcion, Boton, Aviso } from '@netctl/kit';
  import { aplicado, textoError, redesDe } from './estado.svelte.js';
  import { deRegla, aRegla, nuevaRegla, opcionesOrigen, opcionesDestino, DIAS } from './politica.js';
  import { ordenarZonas } from './listaPoliticas.js';

  /** regla: null = nueva (en la pareja origen→destino). */
  let { api, config, regla = null, origen = 'interna', destino = 'externa', oncerrar } = $props();

  const c = $derived(config.config);
  const redes = $derived(redesDe(c));
  const zonas = $derived(ordenarZonas(c.zonas));
  const tarjetasDe = (z) => config.tarjetas_de_zona?.[z] ?? [];
  const gruposIp = $derived(c.grupos_ip.map((g) => ({ id: g.id, nombre: g.nombre })));
  const gruposPuertos = $derived(c.grupos_puertos.map((g) => ({ valor: g.id, texto: g.nombre })));

  let f = $state(untrack(() => deRegla(regla ?? nuevaRegla(origen, destino))));
  let validos = $state({ po: true, pd: true, num: true });
  let ocupado = $state(false);
  let error = $state('');

  const usaPuertos = $derived(['tcp', 'udp', 'tcp_udp'].includes(f.protocolo));

  // Al cambiar de zona, «Red» ya no vale si la tarjeta no es de la zona nueva.
  $effect(() => {
    const t = tarjetasDe(f.origenZona);
    if (f.origen.tipo === 'red' && !t.includes(f.origen.red)) untrack(() => (f.origen = { ...f.origen, tipo: 'cualquiera', red: '' }));
  });
  $effect(() => {
    const t = tarjetasDe(f.destinoZona);
    if (f.destino.tipo === 'red' && !t.includes(f.destino.red)) untrack(() => (f.destino = { ...f.destino, tipo: 'cualquiera', red: '' }));
  });

  /** Elegir puertos con «Todos los protocolos» pasa a TCP y UDP (los puertos solo existen ahí). */
  function modoPuerto(lado, modo) {
    f[lado] = { ...f[lado], modo };
    if (modo !== 'cualquiera' && !usaPuertos) f.protocolo = 'tcp_udp';
    if (modo === 'lista' && f[lado].grupo == null) f[lado].grupo = c.grupos_puertos[0]?.id ?? null;
  }

  const macValida = (m) => /^([0-9a-f]{2}:){5}[0-9a-f]{2}$/i.test((m ?? '').trim());
  const horaValida = (h) => /^([01]\d|2[0-3]):[0-5]\d$/.test(h ?? '');

  const faltas = $derived.by(() => {
    const l = [];
    if (!f.nombre.trim()) l.push('Ponle un nombre.');
    if (f.origenZona === 'gateway' && f.destinoZona === 'gateway') l.push('El origen y el destino no pueden ser los dos «Esta máquina».');
    if (f.origen.tipo === 'mac' && !macValida(f.origen.mac)) l.push('Escribe la MAC como aa:bb:cc:dd:ee:ff.');
    if (['equipo', 'ip'].includes(f.origen.tipo) && !f.origen.dir) l.push('Elige el equipo o escribe la IP de origen.');
    if (f.destino.tipo === 'ip' && !f.destino.dir) l.push('Elige el equipo o escribe la IP de destino.');
    if (f.origen.tipo === 'red' && !f.origen.red) l.push('Elige la tarjeta de origen.');
    if (f.destino.tipo === 'red' && !f.destino.red) l.push('Elige la tarjeta de destino.');
    if (usaPuertos && ((f.puertoOrigen.modo === 'concreto' && !validos.po) || (f.puertoDestino.modo === 'concreto' && !validos.pd))) l.push('Revisa los puertos.');
    if (f.protocolo === 'otro' && !validos.num) l.push('Revisa el número de protocolo.');
    if (f.estado === 'elegir' && !f.estados.length) l.push('Elige al menos un estado de conexión.');
    const franjas = f.horario === 'a_medida' ? f.tramos.map((t) => t.franja) : ['diario', 'semanal'].includes(f.horario) ? [f.franja] : [];
    if (franjas.some((x) => !horaValida(x.desde) || !horaValida(x.hasta))) l.push('Escribe las horas como HH:MM, por ejemplo 08:30.');
    if (f.horario === 'semanal' && !f.dias.length) l.push('Elige al menos un día.');
    if (f.horario === 'a_medida' && f.tramos.some((t) => !t.dias.length)) l.push('Cada franja necesita al menos un día.');
    if (f.horario === 'una_vez' && !(f.inicio && f.fin && f.fin > f.inicio)) l.push('El fin tiene que ser después del inicio.');
    return l;
  });

  async function hacer(accion) {
    ocupado = true;
    error = '';
    try {
      const rev = { revision: c.revision };
      if (accion === 'guardar') {
        const r = aRegla(f);
        aplicado(regla ? await api.put(`/reglas/${regla.id}`, { regla: r, ...rev }) : await api.post('/reglas', { regla: r, ...rev }));
      } else if (accion === 'borrar') aplicado(await api.del(`/reglas/${regla.id}?revision=${c.revision}`));
      else aplicado(await api.post(`/reglas/${regla.id}/duplicar`, rev));
      oncerrar();
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }

  // Fechas de «Una vez» con <input type="datetime-local"> (hora local) ↔ segundos UNIX.
  const aLocal = (s) => {
    if (!s) return '';
    const d = new Date(s * 1000);
    const p = (n) => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`;
  };
  const deLocal = (v) => (v ? Math.floor(new Date(v).getTime() / 1000) : null);

  function alternarDia(lista, d) {
    return lista.includes(d) ? lista.filter((x) => x !== d) : [...lista, d].sort();
  }

  const zonaOpciones = $derived(zonas.map((z) => ({ valor: z.id, texto: z.nombre })));
</script>

{#snippet puertos(lado, titulo)}
  <Radios
    etiqueta="Puerto"
    ayuda={titulo}
    valor={f[lado].modo}
    onchange={(m) => modoPuerto(lado, m)}
    opciones={[
      { valor: 'cualquiera', texto: 'Cualquiera' },
      { valor: 'concreto', texto: 'Concreto', ayuda: 'Uno, varios separados por comas o un rango.' },
      { valor: 'lista', texto: 'Lista', ayuda: 'Un grupo de puertos con nombre (se crean en Grupos).', desactivado: !gruposPuertos.length },
    ]}
  />
  {#if f[lado].modo === 'concreto'}
    <CampoNumero bind:valor={f[lado].texto} bind:valido={validos[lado === 'puertoOrigen' ? 'po' : 'pd']} puertos requerido placeholder="Por ejemplo 443" mas={{ ejemplo: '443  ·  80, 443  ·  8000-8100' }} />
  {:else if f[lado].modo === 'lista'}
    <Campo tipo="select" bind:valor={f[lado].grupo} opciones={gruposPuertos} />
  {/if}
{/snippet}

{#snippet franjaCampos(fr)}
  <div class="dos">
    <Campo etiqueta="Desde" bind:valor={fr.desde} placeholder="08:00" maxlength={5} error={fr.desde && !horaValida(fr.desde) ? 'Usa HH:MM' : ''} />
    <Campo etiqueta="Hasta" bind:valor={fr.hasta} placeholder="17:00" maxlength={5} error={fr.hasta && !horaValida(fr.hasta) ? 'Usa HH:MM' : ''} />
  </div>
{/snippet}

{#snippet diasCampos(lista, cambiar)}
  <div class="dias" role="group" aria-label="Días">
    {#each DIAS as d, i (d)}
      <button type="button" class="dia" class:si={lista.includes(i)} aria-pressed={lista.includes(i)} title={d} onclick={() => cambiar(alternarDia(lista, i))}>{d.slice(0, 2)}</button>
    {/each}
  </div>
{/snippet}

<PanelLateral abierto titulo={regla ? regla.nombre : 'Crear política'} {oncerrar}>
  <Bloque>
    <Campo etiqueta="Nombre" bind:valor={f.nombre} placeholder="Por ejemplo: Cámaras sin internet" ayuda="Para reconocerla en la lista." enfocar={!regla} />
  </Bloque>

  <Bloque titulo="Zona de origen" ayuda="De dónde sale el tráfico. Después puedes afinar a un equipo, una tarjeta, una IP o una MAC de esa zona.">
    <Campo tipo="select" bind:valor={f.origenZona} opciones={zonaOpciones} />
    <Radios bind:valor={f.origen.tipo} opciones={opcionesOrigen(tarjetasDe(f.origenZona))} />
    {#if f.origen.tipo === 'equipo'}
      <SelectorEquipo bind:valor={f.origen.dir} propias={redes.propias} conocidas={redes.conocidas} placeholder="Busca un equipo por nombre, IP o MAC" />
    {:else if f.origen.tipo === 'ip'}
      <SelectorEquipo bind:valor={f.origen.dir} redes grupos={gruposIp} redesLista={redes.lista} conocidas={redes.conocidas} placeholder="IP, red (192.168.1.0/24), rango o grupo" />
    {:else if f.origen.tipo === 'red'}
      <Radios bind:valor={f.origen.red} opciones={tarjetasDe(f.origenZona).map((t) => ({ valor: t, texto: t }))} />
    {:else if f.origen.tipo === 'mac'}
      <Campo bind:valor={f.origen.mac} mono placeholder="aa:bb:cc:dd:ee:ff" error={f.origen.mac && !macValida(f.origen.mac) ? 'Escríbela como aa:bb:cc:dd:ee:ff' : ''} />
    {/if}
    {@render puertos('puertoOrigen', 'El puerto desde el que sale la conexión. Casi nunca hace falta: deja «Cualquiera».')}
  </Bloque>

  <Bloque titulo="Acción" ayuda="Qué hacer con el tráfico que coincide.">
    <Radios
      bind:valor={f.accion}
      opciones={[
        { valor: 'bloquear', texto: 'Bloquear', ayuda: 'Lo tira sin avisar: desde fuera parece que no hay nada.' },
        { valor: 'permitir', texto: 'Permitir' },
        { valor: 'rechazar', texto: 'Rechazar', ayuda: 'Lo corta y avisa al otro lado de que no puede pasar: falla al momento en vez de esperar.' },
      ]}
    />
  </Bloque>

  <Bloque titulo="Zona de destino" ayuda="A dónde va el tráfico.">
    <Campo tipo="select" bind:valor={f.destinoZona} opciones={zonaOpciones} />
    <Radios bind:valor={f.destino.tipo} opciones={opcionesDestino(tarjetasDe(f.destinoZona))} />
    {#if f.destino.tipo === 'ip'}
      <SelectorEquipo bind:valor={f.destino.dir} redes grupos={gruposIp} redesLista={redes.lista} conocidas={redes.conocidas} placeholder="Equipo, IP, red, rango o grupo" />
    {:else if f.destino.tipo === 'red'}
      <Radios bind:valor={f.destino.red} opciones={tarjetasDe(f.destinoZona).map((t) => ({ valor: t, texto: t }))} />
    {/if}
    {@render puertos('puertoDestino', 'El puerto del servicio al que se quiere llegar, por ejemplo 443 para una web.')}
  </Bloque>

  <Bloque>
    <Radios
      etiqueta="Versión IP"
      ayuda="Si la política vale para IPv4, para IPv6 o para las dos."
      bind:valor={f.version}
      opciones={[
        { valor: 'ambas', texto: 'Ambas' },
        { valor: 'v4', texto: 'IPv4' },
        { valor: 'v6', texto: 'IPv6' },
      ]}
    />
    <Radios
      etiqueta="Protocolo"
      ayuda="TCP lo usan las webs y casi todo; UDP los juegos, la voz y el DNS. Si no lo sabes, deja «Todos»."
      bind:valor={f.protocolo}
      opciones={[
        { valor: 'cualquiera', texto: 'Todos' },
        { valor: 'tcp_udp', texto: 'TCP y UDP' },
        { valor: 'tcp', texto: 'TCP' },
        { valor: 'udp', texto: 'UDP' },
        { valor: 'icmp', texto: 'ICMP (ping)' },
        { valor: 'otro', texto: 'Otro', ayuda: 'Otro protocolo por su número, por ejemplo 47 (GRE) o 50 (IPsec ESP).' },
      ]}
    />
    {#if f.protocolo === 'otro'}
      <CampoNumero etiqueta="Número de protocolo" bind:valor={f.numero} bind:valido={validos.num} min={0} max={255} requerido mas={{ ejemplo: '47 (GRE)  ·  50 (ESP)  ·  132 (SCTP)' }} />
    {/if}
    {#if !usaPuertos && (f.puertoOrigen.modo !== 'cualquiera' || f.puertoDestino.modo !== 'cualquiera')}
      <span class="nota">Con este protocolo no hay puertos: los que elegiste arriba no se usan.</span>
    {/if}
    <Radios
      etiqueta="Estado de la conexión"
      ayuda="«Todas» vale casi siempre. «Solo nuevas» mira solo el primer paquete de cada conexión. Con «A elegir» y «Establecida» también puedes cortar conexiones que ya estaban abiertas."
      bind:valor={f.estado}
      opciones={[
        { valor: 'todas', texto: 'Todas' },
        { valor: 'solo_nuevas', texto: 'Solo nuevas' },
        { valor: 'elegir', texto: 'A elegir' },
      ]}
    />
    {#if f.estado === 'elegir'}
      <div class="marcas">
        {#each [['nueva', 'Nueva'], ['establecida', 'Establecida'], ['relacionada', 'Relacionada'], ['invalida', 'Inválida']] as [v, t] (v)}
          <label><input type="checkbox" checked={f.estados.includes(v)} onchange={() => (f.estados = f.estados.includes(v) ? f.estados.filter((x) => x !== v) : [...f.estados, v])} /> {t}</label>
        {/each}
      </div>
    {/if}
    <Opcion titulo="Registrar (log)" texto="Anota en el registro del sistema cada vez que se cumpla. Solo para investigar." bind:activo={f.log} />
    <Radios
      etiqueta="Horario"
      ayuda="Cuándo se aplica. Fuera de ese horario es como si no existiera."
      bind:valor={f.horario}
      opciones={[
        { valor: 'siempre', texto: 'Siempre' },
        { valor: 'diario', texto: 'Diario' },
        { valor: 'semanal', texto: 'Semanal' },
        { valor: 'una_vez', texto: 'Una vez' },
        { valor: 'a_medida', texto: 'A medida', ayuda: 'Varias franjas, cada una con sus días.' },
      ]}
    />
    {#if f.horario === 'diario'}
      {@render franjaCampos(f.franja)}
    {:else if f.horario === 'semanal'}
      {@render diasCampos(f.dias, (d) => (f.dias = d))}
      {@render franjaCampos(f.franja)}
    {:else if f.horario === 'una_vez'}
      <div class="dos">
        <Campo etiqueta="Inicio" tipo="datetime-local" valor={aLocal(f.inicio)} oninput={(e) => (f.inicio = deLocal(e.currentTarget.value))} />
        <Campo etiqueta="Fin" tipo="datetime-local" valor={aLocal(f.fin)} oninput={(e) => (f.fin = deLocal(e.currentTarget.value))} />
      </div>
    {:else if f.horario === 'a_medida'}
      {#each f.tramos as t, i (i)}
        <div class="tramo">
          {@render diasCampos(t.dias, (d) => (f.tramos[i].dias = d))}
          {@render franjaCampos(t.franja)}
          {#if f.tramos.length > 1}<Boton tam="pequeno" icono="papelera" onclick={() => (f.tramos = f.tramos.filter((_, j) => j !== i))}>Quitar franja</Boton>{/if}
        </div>
      {/each}
      <Boton tam="pequeno" icono="mas" onclick={() => (f.tramos = [...f.tramos, { dias: [0, 1, 2, 3, 4], franja: { desde: '08:00', hasta: '17:00' } }])}>Añadir franja</Boton>
    {/if}
    <Campo etiqueta="Descripción" tipo="textarea" filas={3} bind:valor={f.descripcion} ayuda="Para acordarte de por qué la pusiste." />
    <Opcion titulo="Activa" texto="Apágala para dejarla en pausa sin borrarla." bind:activo={f.activa} />
  </Bloque>

  {#if error}<Aviso tipo="error">{error}</Aviso>{/if}

  {#snippet pie()}
    {#if regla}
      <Boton variante="peligro" icono="papelera" desactivado={ocupado} onclick={() => hacer('borrar')}>Borrar</Boton>
      <Boton icono="copiar" desactivado={ocupado} titulo="Copia en pausa para cambiarla" onclick={() => hacer('duplicar')}>Duplicar</Boton>
    {/if}
    <span class="hueco"></span>
    <Boton onclick={oncerrar}>Cancelar</Boton>
    <Boton variante="primario" cargando={ocupado} desactivado={faltas.length > 0} titulo={faltas.join(' ')} onclick={() => hacer('guardar')}>Guardar</Boton>
  {/snippet}
</PanelLateral>

<style>
  .dos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .dias {
    display: flex;
    gap: 4px;
  }
  .dia {
    width: 34px;
    height: 28px;
    border: 0;
    border-radius: var(--radio-pq);
    background: var(--campo);
    color: var(--texto-2);
    font: inherit;
    cursor: pointer;
  }
  .dia.si {
    background: var(--azul);
    color: #fff;
  }
  .tramo {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
    border: 1px solid var(--borde-suave);
    border-radius: var(--radio-pq);
  }
  .marcas {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
  }
  .marcas label {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    cursor: pointer;
  }
  .nota {
    color: var(--texto-2);
    font-size: 12px;
  }
  .hueco {
    flex: 1;
  }
</style>
