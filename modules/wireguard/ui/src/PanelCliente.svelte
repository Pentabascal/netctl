<script>
  // Panel lateral de un cliente: General (conectar con QR / .conf), Acceso y Avanzado.
  import { untrack } from 'svelte';
  import { PanelLateral, Pestanas, Campo, Boton, Chip, Aviso, Interruptor, Icono, formatoBytes, formatoHace } from '@netctl/kit';
  import Opcion from './Opcion.svelte';
  import { estadoCliente, formatoTasa, copiar } from './util.js';

  let { api, cliente = null, nuevo = false, intranet = [], revision, ahora, navegar, bienvenida = false, caida = false, oncambiado, oncerrar } = $props();

  let abierto = $state(true);
  let pestana = $state('general');
  let form = $state(vacio());
  let base = $state('');
  let error = $state('');
  let errores = $state([]);
  let conflicto = $state(false);
  let ok = $state('');
  let guardando = $state(false);
  let confirmar = $state(''); // 'regenerar' | 'borrar'
  let ocupado = $state(false);
  let copiado = $state(false);

  function vacio() {
    return { nombre: '', ip: '', internet: false, toda_intranet: true, subredes: [], extra: '', usar_dns: false, usar_psk: true, keepalive: '25' };
  }

  function desde(c) {
    if (!c) return vacio();
    const { nombre, ip, internet, toda_intranet, subredes, extra, usar_dns, usar_psk, keepalive } = c;
    return { nombre, ip, internet, toda_intranet, subredes: [...subredes], extra, usar_dns, usar_psk, keepalive };
  }

  function reiniciar() {
    form = desde(nuevo ? null : cliente);
    base = JSON.stringify(form);
    error = '';
    errores = [];
    conflicto = false;
    confirmar = '';
  }

  // Al cambiar de cliente (o pasar de «nuevo» al cliente creado) se recarga el formulario.
  const clave = $derived(nuevo ? 'nuevo' : cliente?.indice);
  let claveAnterior;
  $effect(() => {
    const k = clave;
    untrack(() => {
      if (k !== claveAnterior) {
        if (claveAnterior !== undefined && claveAnterior !== 'nuevo') pestana = 'general';
        ok =
          claveAnterior === 'nuevo' && k !== 'nuevo'
            ? 'Cliente creado. Escanea el código con el móvil o descarga el fichero.'
            : claveAnterior === undefined && bienvenida
              ? 'VPN creada. Escanea el código con la app WireGuard del móvil y ya estás dentro.'
              : '';
        claveAnterior = k;
        reiniciar();
      }
    });
  });

  const sucio = $derived(JSON.stringify(form) !== base);
  const est = $derived(cliente && !nuevo ? estadoCliente(cliente, caida) : null);

  const pestanas = $derived(
    nuevo
      ? [
          { id: 'general', titulo: 'General' },
          { id: 'acceso', titulo: 'Acceso' },
        ]
      : [
          { id: 'general', titulo: 'General' },
          { id: 'acceso', titulo: 'Acceso' },
          { id: 'avanzado', titulo: 'Avanzado' },
        ],
  );

  // Resumen de a qué puede llegar (lo mismo que calcula el backend en `acceso`).
  const alcance = $derived.by(() => {
    const p = ['la red VPN'];
    if (form.toda_intranet) {
      if (intranet.length) p.push('toda la intranet');
    } else {
      for (const s of intranet) if (form.subredes.includes(s.id)) p.push(s.nombre);
    }
    if (form.extra.trim()) p.push(form.extra.trim());
    if (form.internet) p.push('internet');
    return p.length > 1 ? p.slice(0, -1).join(', ') + ' y ' + p.at(-1) : p[0];
  });

  function errorDe(e) {
    error = e.mensaje;
    errores = e.datos?.errores?.length > 1 ? e.datos.errores : [];
    conflicto = e.estado === 409;
  }

  async function guardar(ev) {
    ev?.preventDefault();
    guardando = true;
    error = '';
    errores = [];
    ok = '';
    try {
      const cuerpo = { revision, ...form };
      if (nuevo && !cuerpo.ip.trim()) delete cuerpo.ip;
      if (nuevo) {
        const r = await api.post('/clientes', cuerpo);
        await oncambiado(r.indice);
      } else {
        await api.put(`/clientes/${cliente.indice}`, cuerpo);
        base = JSON.stringify(form);
        await oncambiado();
        ok = 'Guardado. Los demás clientes siguen conectados.';
      }
    } catch (e) {
      errorDe(e);
    } finally {
      guardando = false;
    }
  }

  async function accion(fn, mensaje) {
    ocupado = true;
    error = '';
    ok = '';
    try {
      await fn();
      if (mensaje) ok = mensaje;
      confirmar = '';
      await oncambiado();
    } catch (e) {
      errorDe(e);
    } finally {
      ocupado = false;
    }
  }

  const alternarActivo = (v) =>
    accion(
      () => api.post(`/clientes/${cliente.indice}/${v ? 'activar' : 'desactivar'}`, { revision }),
      v ? 'Activado: ya puede conectarse.' : 'Desactivado: no puede conectarse, pero se conservan sus datos.',
    );

  const regenerar = () =>
    accion(
      () => api.post(`/clientes/${cliente.indice}/regenerar-claves`, { revision }),
      'Claves nuevas creadas. Vuelve a importar la configuración en el dispositivo.',
    );

  async function borrar() {
    ocupado = true;
    try {
      await api.del(`/clientes/${cliente.indice}?revision=${revision}`);
      cerrar();
      await oncambiado();
    } catch (e) {
      errorDe(e);
    } finally {
      ocupado = false;
    }
  }

  async function recargar() {
    await oncambiado();
    reiniciar();
  }

  function cerrar() {
    abierto = false;
    oncerrar();
  }

  function alternarSubred(id) {
    form.subredes = form.subredes.includes(id) ? form.subredes.filter((s) => s !== id) : [...form.subredes, id];
  }

  async function copiarClave() {
    copiado = await copiar(cliente.clave_publica);
    setTimeout(() => (copiado = false), 1500);
  }

  const conf = $derived(cliente && !nuevo ? api.url(`/clientes/${cliente.indice}/conf`) : '');
  const qr = $derived(cliente && !nuevo ? api.url(`/clientes/${cliente.indice}/qr.svg`) + `?r=${revision}` : '');
</script>

<PanelLateral
  bind:abierto
  titulo={nuevo ? 'Nuevo cliente' : cliente.nombre}
  subtitulo={nuevo ? 'Uno por cada dispositivo que se conecte' : `${cliente.ip} · ${cliente.acceso}`}
  icono={nuevo ? 'mas' : 'usuario'}
  ancho="480px"
  oncerrar={cerrar}
>
  {#snippet cabecera()}
    {#if !nuevo}
      <div class="estado">
        <Chip color={est.color} punto={est.clave !== 'desactivado'}>{est.texto}</Chip>
        {#if cliente.activo && cliente.estado.ultimo_handshake}
          <span class="hace">Último saludo {formatoHace(cliente.estado.ultimo_handshake, ahora)}</span>
        {/if}
        <span class="nc-separador"></span>
        <label class="activo">
          <span>Activo</span>
          <Interruptor activo={cliente.activo} desactivado={ocupado} etiqueta="Cliente activo" onchange={alternarActivo} />
        </label>
      </div>
    {/if}
    <Pestanas {pestanas} bind:activa={pestana} />
  {/snippet}

  <form id="form-cliente" class="cuerpo" onsubmit={guardar}>
    {#if error}
      <Aviso tipo="error">
        {error}
        {#if errores.length}
          <ul>
            {#each errores as x (x)}<li>{x}</li>{/each}
          </ul>
        {/if}
        {#snippet acciones()}
          {#if conflicto}<Boton tam="pequeno" icono="actualizar" onclick={recargar}>Recargar</Boton>{/if}
        {/snippet}
      </Aviso>
    {/if}
    {#if ok}<Aviso tipo="ok" cerrable oncerrar={() => (ok = '')}>{ok}</Aviso>{/if}

    {#if pestana === 'general'}
      {#if !nuevo}
        <section class="conectar">
          <div class="qr">
            {#if cliente.activo}
              <img src={qr} alt="Código QR con la configuración de {cliente.nombre}" width="168" height="168" />
            {:else}
              <div class="qr-apagado"><Icono nombre="candado" tam={22} /><span>Activa el cliente para conectarlo</span></div>
            {/if}
          </div>
          <div class="conectar-texto">
            <h3>Conectar el dispositivo</h3>
            <p>En el móvil, abre la app WireGuard y escanea el código. En un ordenador, importa el fichero.</p>
            <Boton icono="descargar" href={conf} descargar={true} desactivado={!cliente.activo}>Descargar .conf</Boton>
          </div>
        </section>

        {#if cliente.activo && cliente.estado.ultimo_handshake}
          <div class="datos">
            <div><span class="etq">Origen</span><span class="mono">{cliente.estado.endpoint ?? '—'}</span></div>
            <div><span class="etq">Velocidad</span><span>↓ {formatoTasa(cliente.estado.tasa_rx)} · ↑ {formatoTasa(cliente.estado.tasa_tx)}</span></div>
            <div><span class="etq">Recibido</span><span>{formatoBytes(cliente.estado.rx)}</span></div>
            <div><span class="etq">Enviado</span><span>{formatoBytes(cliente.estado.tx)}</span></div>
          </div>
        {/if}
      {/if}

      <Campo
        etiqueta="Nombre"
        bind:valor={form.nombre}
        placeholder="Móvil de Ana"
        ayuda={nuevo ? 'Para reconocerlo en la lista. Puedes usar espacios y tildes.' : ''}
        enfocar={nuevo}
        requerido
      />
      <Campo
        etiqueta="IP en la VPN"
        bind:valor={form.ip}
        mono
        placeholder={nuevo ? 'Automática' : ''}
        ayuda={nuevo ? 'Déjala vacía y se asigna la siguiente libre.' : 'Cámbiala solo si lo necesitas: el dispositivo tendrá que importar la configuración otra vez.'}
      />
      {#if nuevo}
        <div class="resumen"><Icono nombre="info" tam={15} /><span>Podrá llegar a <strong>{alcance}</strong>. Cámbialo en «Acceso».</span></div>
      {/if}
    {:else if pestana === 'acceso'}
      <div class="bloque">
        <Opcion
          titulo="Internet a través de la VPN"
          texto="Todo su tráfico sale por este servidor (útil en wifis públicas). Las redes privadas que no le permitas siguen bloqueadas."
          bind:activo={form.internet}
        />
        <Opcion
          titulo="Usar el DNS del servidor"
          texto="El dispositivo resolverá nombres con los DNS configurados en el servidor."
          bind:activo={form.usar_dns}
        />
      </div>

      <div class="grupo">
        <h3 class="nc-seccion">Intranet</h3>
        {#if intranet.length === 0}
          <p class="nota">
            No hay redes de intranet definidas.
            <button type="button" class="enlace" onclick={() => navegar('servidor')}>Añádelas en Servidor › Intranet</button>.
          </p>
        {:else}
          <div class="radios" role="radiogroup" aria-label="Acceso a la intranet">
            <button type="button" role="radio" aria-checked={form.toda_intranet} class:elegido={form.toda_intranet} onclick={() => (form.toda_intranet = true)}>
              <span class="radio"></span>
              <span><strong>Toda la intranet</strong><small>{intranet.length} {intranet.length === 1 ? 'red' : 'redes'}, también las que añadas después</small></span>
            </button>
            <button type="button" role="radio" aria-checked={!form.toda_intranet} class:elegido={!form.toda_intranet} onclick={() => (form.toda_intranet = false)}>
              <span class="radio"></span>
              <span><strong>Solo algunas redes</strong><small>Elige a cuáles puede llegar</small></span>
            </button>
          </div>
          {#if !form.toda_intranet}
            <div class="subredes">
              {#each intranet as s (s.id)}
                <label class="subred">
                  <input type="checkbox" checked={form.subredes.includes(s.id)} onchange={() => alternarSubred(s.id)} />
                  <span class="caja"><Icono nombre="ok" tam={12} trazo={2.5} /></span>
                  <span class="s-nombre">{s.nombre}</span>
                  <span class="mono s-cidr">{s.cidr}</span>
                </label>
              {/each}
            </div>
          {/if}
        {/if}
      </div>

      <Campo
        etiqueta="Otras redes o IPs"
        bind:valor={form.extra}
        mono
        placeholder="10.20.0.0/16, 172.16.5.10"
        ayuda="Además de las de arriba, separadas por comas."
      />

      <div class="resumen"><Icono nombre="escudo" tam={15} /><span>Podrá llegar a <strong>{alcance}</strong>. Todo lo demás queda bloqueado.</span></div>
    {:else}
      <div class="bloque">
        <Opcion
          titulo="Clave precompartida (PSK)"
          texto="Una capa más de cifrado. Recomendada; quitarla obliga a importar la configuración otra vez."
          bind:activo={form.usar_psk}
        />
      </div>
      <Campo
        etiqueta="Keepalive"
        bind:valor={form.keepalive}
        sufijo="s"
        inputmode="numeric"
        ayuda="Mantiene abierta la conexión detrás de routers con NAT. 25 s va bien; 0 lo desactiva."
      />
      <div class="nc-campo-falso">
        <span class="etq">Clave pública</span>
        <div class="clave">
          <code>{cliente.clave_publica}</code>
          <Boton variante="fantasma" tam="pequeno" icono={copiado ? 'ok' : 'copiar'} titulo="Copiar clave pública" onclick={copiarClave} />
        </div>
      </div>

      <div class="peligro">
        <h3 class="nc-seccion">Zona delicada</h3>
        {#if confirmar === 'regenerar'}
          <Aviso tipo="aviso" titulo="¿Regenerar las claves de {cliente.nombre}?">
            El dispositivo dejará de conectar hasta que importe la configuración nueva.
            {#snippet acciones()}
              <Boton tam="pequeno" variante="fantasma" onclick={() => (confirmar = '')}>Cancelar</Boton>
              <Boton tam="pequeno" variante="peligro" cargando={ocupado} onclick={regenerar}>Regenerar</Boton>
            {/snippet}
          </Aviso>
        {:else if confirmar === 'borrar'}
          <Aviso tipo="error" titulo="¿Borrar {cliente.nombre}?">
            Se quita de la VPN y se pierden sus claves. Si solo quieres cortarle el acceso, desactívalo.
            {#snippet acciones()}
              <Boton tam="pequeno" variante="fantasma" onclick={() => (confirmar = '')}>Cancelar</Boton>
              <Boton tam="pequeno" variante="peligro" cargando={ocupado} onclick={borrar}>Borrar</Boton>
            {/snippet}
          </Aviso>
        {:else}
          <div class="fila-peligro">
            <div><strong>Regenerar claves</strong><small>Si crees que el fichero .conf se ha filtrado.</small></div>
            <Boton icono="actualizar" onclick={() => (confirmar = 'regenerar')}>Regenerar</Boton>
          </div>
          <div class="fila-peligro">
            <div><strong>Borrar cliente</strong><small>No se puede deshacer.</small></div>
            <Boton variante="peligro" icono="papelera" onclick={() => (confirmar = 'borrar')}>Borrar</Boton>
          </div>
        {/if}
      </div>
    {/if}
  </form>

  {#snippet pie()}
    <Boton variante="fantasma" onclick={sucio && !nuevo ? reiniciar : cerrar}>{sucio && !nuevo ? 'Descartar' : 'Cerrar'}</Boton>
    <Boton variante="primario" cargando={guardando} desactivado={!nuevo && !sucio} onclick={guardar}>
      {nuevo ? 'Crear cliente' : 'Guardar'}
    </Boton>
  {/snippet}
</PanelLateral>

<style>
  .estado {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 0 4px;
    flex-wrap: wrap;
  }
  .hace {
    color: var(--texto-2);
    font-size: 12.5px;
  }
  .activo {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--texto-2);
    font-size: 13px;
  }
  .cuerpo {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .conectar {
    display: flex;
    gap: 18px;
    align-items: center;
    padding: 16px;
    border-radius: var(--radio);
    background: var(--superficie-2);
    border: 1px solid var(--borde-suave);
  }
  .qr {
    flex: none;
  }
  .qr img {
    display: block;
    width: 168px;
    height: 168px;
    border-radius: 6px;
    background: #fff;
  }
  .qr-apagado {
    width: 168px;
    height: 168px;
    border-radius: 6px;
    border: 1px dashed var(--borde-fuerte);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 16px;
    text-align: center;
    color: var(--texto-3);
    font-size: 12.5px;
  }
  .conectar-texto {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }
  .conectar-texto p {
    color: var(--texto-2);
    font-size: 13px;
    line-height: 1.5;
  }
  .datos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px 16px;
  }
  .datos div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .etq {
    font-size: 12px;
    color: var(--texto-3);
  }
  .mono {
    font-family: var(--mono);
    font-size: 12.5px;
    overflow-wrap: anywhere;
  }
  .bloque {
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    padding: 2px 14px;
  }
  .grupo .nc-seccion {
    margin-bottom: 8px;
  }
  .nota {
    color: var(--texto-2);
    font-size: 13px;
  }
  .enlace {
    background: none;
    border: 0;
    padding: 0;
    font: inherit;
    color: var(--azul);
    cursor: pointer;
  }
  .radios {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .radios button {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    text-align: left;
    padding: 10px 12px;
    border-radius: var(--radio);
    border: 1px solid var(--borde);
    background: var(--campo);
    color: var(--texto);
    font: inherit;
    cursor: pointer;
  }
  .radios button span:last-child {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .radios strong {
    font-weight: 500;
  }
  .radios small {
    color: var(--texto-3);
    font-size: 12px;
  }
  .radios .elegido {
    border-color: var(--azul);
    background: color-mix(in srgb, var(--azul-tinte) 45%, var(--campo));
  }
  .radio {
    width: 16px;
    height: 16px;
    flex: none;
    margin-top: 2px;
    border-radius: 50%;
    border: 1.5px solid var(--borde-fuerte);
  }
  .elegido .radio {
    border: 5px solid var(--azul);
  }
  .subredes {
    margin-top: 8px;
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    overflow: hidden;
  }
  .subred {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    cursor: pointer;
  }
  .subred + .subred {
    border-top: 1px solid var(--borde-suave);
  }
  .subred:hover {
    background: var(--hover);
  }
  .subred input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .caja {
    width: 16px;
    height: 16px;
    flex: none;
    border-radius: 4px;
    border: 1.5px solid var(--borde-fuerte);
    display: grid;
    place-items: center;
    color: transparent;
  }
  .subred input:checked + .caja {
    background: var(--azul);
    border-color: var(--azul);
    color: #fff;
  }
  .subred input:focus-visible + .caja {
    outline: 2px solid var(--azul);
    outline-offset: 2px;
  }
  .s-nombre {
    flex: 1;
  }
  .s-cidr {
    color: var(--texto-2);
  }
  .resumen {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 10px 12px;
    border-radius: var(--radio);
    background: var(--campo);
    border: 1px solid var(--borde-suave);
    color: var(--texto-2);
    font-size: 13px;
    line-height: 1.5;
  }
  .resumen :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--azul);
  }
  .resumen strong {
    color: var(--texto);
    font-weight: 500;
  }
  .nc-campo-falso {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .nc-campo-falso .etq {
    font-size: 13px;
    color: var(--texto-2);
  }
  .clave {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .clave code {
    flex: 1;
    min-width: 0;
    padding: 7px 10px;
    overflow-wrap: anywhere;
    color: var(--texto-2);
  }
  .peligro {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 6px;
  }
  .peligro .nc-seccion {
    margin: 0;
  }
  .fila-peligro {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 14px;
    border: 1px solid var(--borde);
    border-radius: var(--radio);
  }
  .fila-peligro div {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .fila-peligro strong {
    font-weight: 500;
  }
  .fila-peligro small {
    color: var(--texto-3);
    font-size: 12px;
  }
  ul {
    margin: 6px 0 0;
    padding-left: 18px;
  }
  @media (max-width: 520px) {
    .conectar {
      flex-direction: column;
      align-items: stretch;
      text-align: center;
    }
    .qr {
      align-self: center;
    }
    .conectar-texto {
      align-items: center;
    }
    .radios {
      grid-template-columns: 1fr;
    }
  }
</style>
