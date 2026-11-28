<script>
  // Formulario de una regla (dentro del panel de la casilla). Todos los campos a la vista,
  // del más usado al menos, cada uno con su explicación (§3b).
  import { untrack } from 'svelte';
  import { Campo, CampoNumero, SelectorEquipo, Opcion, Boton, Aviso } from '@netctl/kit';
  import { aplicado, textoError, textoPuertos, redesDe } from './estado.svelte.js';

  let { api, config, origen, destino, regla = null, onhecho } = $props();

  const c = $derived(config.config);
  const redes = $derived(redesDe(c));
  const gruposIp = $derived(c.grupos_ip.map((g) => ({ id: g.id, nombre: g.nombre })));

  // Valores iniciales del formulario (se capturan al abrir, a propósito).
  const base = untrack(() => regla ?? {
    nombre: '',
    accion: destino === 'gateway' || origen === 'externa' ? 'permitir' : 'bloquear',
    protocolo: 'tcp',
    puertos: { tipo: 'lista', valor: [] },
    origen: { tipo: 'cualquiera' },
    destino: { tipo: 'cualquiera' },
    log: false,
    activa: true,
    notas: '',
  });
  let nombre = $state(base.nombre);
  let accion = $state(base.accion);
  let protocolo = $state(base.protocolo);
  let usarGrupoPuertos = $state(base.puertos?.tipo === 'grupo');
  let textoP = $state(base.puertos?.tipo === 'lista' ? textoPuertos(base.puertos.valor) : '');
  let grupoP = $state(base.puertos?.tipo === 'grupo' ? base.puertos.valor : untrack(() => c.grupos_puertos[0]?.id ?? null));
  let puertosValidos = $state(true);
  let dirOrigen = $state(base.origen?.tipo === 'cualquiera' ? null : base.origen);
  let dirDestino = $state(base.destino?.tipo === 'cualquiera' ? null : base.destino);
  let log = $state(base.log);
  let activa = $state(base.activa);
  let notas = $state(base.notas);
  let ocupado = $state(false);
  let error = $state('');

  const usaPuertos = $derived(['tcp', 'udp', 'tcp_udp'].includes(protocolo));
  const sinNombre = $derived(!nombre.trim());

  function puertos() {
    if (!usaPuertos) return { tipo: 'cualquiera' };
    if (usarGrupoPuertos) return grupoP != null ? { tipo: 'grupo', valor: grupoP } : { tipo: 'cualquiera' };
    if (!textoP.trim()) return { tipo: 'cualquiera' };
    const valor = textoP
      .split(',')
      .map((x) => x.trim())
      .filter(Boolean)
      .map((p) => {
        const [a, b = a] = p.split('-').map(Number);
        return { desde: a, hasta: b };
      });
    return { tipo: 'lista', valor };
  }

  async function guardar() {
    ocupado = true;
    error = '';
    const datos = {
      ...(regla ?? {}),
      nombre: nombre.trim(),
      origen_zona: origen,
      destino_zona: destino,
      accion,
      protocolo,
      puertos: puertos(),
      origen: dirOrigen ?? { tipo: 'cualquiera' },
      destino: dirDestino ?? { tipo: 'cualquiera' },
      log,
      activa,
      notas,
    };
    try {
      const r = regla
        ? await api.put(`/reglas/${regla.id}`, { regla: datos, revision: c.revision })
        : await api.post('/reglas', { regla: datos, revision: c.revision });
      aplicado(r);
      onhecho?.();
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }

  async function borrar() {
    ocupado = true;
    error = '';
    try {
      aplicado(await api.del(`/reglas/${regla.id}?revision=${c.revision}`));
      onhecho?.();
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }

  const ACCIONES = [
    { id: 'permitir', titulo: 'Permitir', texto: 'Deja pasar el tráfico.' },
    { id: 'bloquear', titulo: 'Bloquear (drop)', texto: 'Lo descarta en silencio: el otro lado no sabe nada.' },
    { id: 'rechazar', titulo: 'Rechazar (reject)', texto: 'Lo corta y avisa al otro lado; útil dentro de tu red.' },
  ];
</script>

<div class="form">
  <Campo
    etiqueta="Nombre"
    bind:valor={nombre}
    placeholder="Web desde la oficina"
    ayuda="Para reconocerla en la lista."
    requerido
    enfocar={!regla}
  />

  <div class="campo">
    <span class="etq">Qué hacer</span>
    <div class="acciones" role="radiogroup" aria-label="Qué hacer">
      {#each ACCIONES as a (a.id)}
        <button type="button" role="radio" aria-checked={accion === a.id} class:elegida={accion === a.id} onclick={() => (accion = a.id)}>
          <b>{a.titulo}</b><small>{a.texto}</small>
        </button>
      {/each}
    </div>
  </div>

  <Campo
    etiqueta="Protocolo"
    tipo="select"
    bind:valor={protocolo}
    opciones={[
      { valor: 'tcp_udp', texto: 'TCP y UDP' },
      { valor: 'tcp', texto: 'TCP (web, SSH, la mayoría)' },
      { valor: 'udp', texto: 'UDP (juegos, VPN, voz)' },
      { valor: 'icmp', texto: 'ICMP (ping)' },
      { valor: 'cualquiera', texto: 'Cualquiera (todo el tráfico)' },
    ]}
    ayuda="Si no lo sabes, deja TCP y UDP."
    mas={{ texto: 'El tipo de conexión. Cada servicio indica cuál usa; muchos usan solo TCP.', ejemplo: 'Una web usa TCP 80 y 443', recomendado: 'TCP y UDP si dudas' }}
  />

  {#if usaPuertos}
    {#if c.grupos_puertos.length}
      <Opcion titulo="Usar un grupo de puertos" texto="Una lista de puertos guardada en Grupos, para no repetirla en cada regla." bind:activo={usarGrupoPuertos} />
    {/if}
    {#if usarGrupoPuertos}
      <Campo etiqueta="Grupo de puertos" tipo="select" bind:valor={grupoP} opciones={c.grupos_puertos.map((g) => ({ valor: g.id, texto: `${g.nombre} (${textoPuertos(g.puertos)})` }))} />
    {:else}
      <CampoNumero
        etiqueta="Puertos de destino"
        bind:valor={textoP}
        bind:valido={puertosValidos}
        puertos
        placeholder="Todos"
        ayuda="Uno, varios separados por comas o un rango. Vacío = todos los puertos."
        mas={{ texto: 'El puerto en el que escucha el servicio al que se quiere llegar.', ejemplo: '443  ·  80, 443  ·  8000-8100' }}
      />
    {/if}
  {/if}

  <SelectorEquipo
    etiqueta="Desde (origen)"
    bind:valor={dirOrigen}
    redes
    grupos={gruposIp}
    redesLista={redes.lista}
    conocidas={redes.conocidas}
    placeholder="Cualquiera: busca un equipo o escribe una IP, red o rango"
    ayuda="Vacío = cualquier equipo de la zona de origen."
    mas={{ texto: 'Si eliges un equipo de la lista, la regla le sigue aunque cambie de IP.', ejemplo: '192.168.1.0/24  ·  192.168.1.10-192.168.1.20' }}
  />
  <SelectorEquipo
    etiqueta="Hacia (destino)"
    bind:valor={dirDestino}
    redes
    grupos={gruposIp}
    redesLista={redes.lista}
    conocidas={redes.conocidas}
    placeholder="Cualquiera: busca un equipo o escribe una IP, red o rango"
    ayuda="Vacío = cualquier equipo de la zona de destino."
  />

  <Opcion
    titulo="Activa"
    texto="Apágala para dejar de aplicarla sin borrarla."
    bind:activo={activa}
  />
  <Opcion
    titulo="Registrar coincidencias (log)"
    texto="Guarda en el registro del sistema cada paquete que coincida. Úsalo solo para investigar: genera mucho texto."
    bind:activo={log}
    mas={{ texto: 'Los mensajes aparecen en el registro del sistema (journalctl -k) con el prefijo «netctl».', recomendado: 'desactivado' }}
  />
  <Campo etiqueta="Notas" tipo="textarea" filas={2} bind:valor={notas} placeholder="Por qué existe esta regla" ayuda="Opcional, solo para ti." />

  {#if error}<Aviso tipo="error">{error}</Aviso>{/if}
  <div class="pie">
    {#if regla}<Boton variante="peligro" icono="papelera" desactivado={ocupado} onclick={borrar}>Borrar</Boton>{/if}
    <span class="hueco"></span>
    <Boton onclick={onhecho}>Cancelar</Boton>
    <Boton variante="primario" cargando={ocupado} desactivado={sinNombre || !puertosValidos} onclick={guardar}>Guardar</Boton>
  </div>
</div>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .campo {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .etq {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--texto-2);
  }
  .acciones {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
  }
  .acciones button {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 9px 10px;
    text-align: left;
    background: var(--superficie-2);
    border: 1px solid var(--borde);
    border-radius: var(--radio-pq);
    color: var(--texto);
    font: inherit;
    cursor: pointer;
  }
  .acciones button.elegida {
    border-color: var(--azul);
    background: var(--azul-tinte);
  }
  .acciones b {
    font-weight: 500;
    font-size: 13px;
  }
  .acciones small {
    color: var(--texto-3);
    font-size: 11.5px;
    line-height: 1.35;
  }
  .pie {
    display: flex;
    gap: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--borde-suave);
  }
  .hueco {
    flex: 1;
  }
</style>
