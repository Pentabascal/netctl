<script>
  // WireGuard › Servidor: General / Intranet / Conexión y claves. Los cambios que reinician la VPN se avisan antes.
  import { onMount } from 'svelte';
  import { Barra, Pestanas, Tarjeta, Campo, Boton, Aviso, Chip, Icono, Opcion, CampoNumero } from '@netctl/kit';
  import { CAMPOS_REINICIO, copiar } from './util.js';

  let { api, navegar } = $props();

  let datos = $state(null);
  let vivo = $state(null); // { interfaz_activa, metricas }
  let form = $state(null);
  let base = $state('');
  let pestana = $state('general');
  let error = $state('');
  let errores = $state([]);
  let conflicto = $state(false);
  let ok = $state('');
  let guardando = $state(false);
  let copiado = $state(false);
  let confirmar = $state(''); // 'claves' | 'importar'
  let ocupado = $state(false);

  // Intranet
  let nuevaNombre = $state('');
  let nuevaCidr = $state('');
  let errorRed = $state('');
  let anadiendo = $state(false);

  // Importar
  let ficheroImportar = $state(null);
  let entradaFichero = $state();

  async function cargar() {
    try {
      const [s, c] = await Promise.all([api.get('/servidor'), api.get('/clientes')]);
      datos = s;
      vivo = c;
      const { servidor: v } = s;
      form = {
        interfaz: v.interfaz,
        endpoint: v.endpoint,
        puerto: v.puerto,
        red_vpn: v.red_vpn,
        ip_servidor: v.ip_servidor,
        iface_salida: v.iface_salida,
        dns: v.dns,
        mtu: v.mtu,
        nat: v.nat,
        firewall: v.firewall,
        entre_clientes: v.entre_clientes,
      };
      base = JSON.stringify(form);
      conflicto = false;
    } catch (e) {
      error = e.mensaje;
    }
  }

  onMount(cargar);

  const sucio = $derived(form != null && JSON.stringify(form) !== base);
  const reinicia = $derived.by(() => {
    if (!form || !base) return false;
    const b = JSON.parse(base);
    return CAMPOS_REINICIO.some((k) => String(form[k]).trim() !== String(b[k]).trim());
  });
  const conectados = $derived(vivo?.metricas?.conectados ?? 0);
  const s = $derived(datos?.servidor);

  const interfaces = $derived.by(() => {
    const v = new Set([...(datos?.interfaces ?? []), form?.iface_salida].filter(Boolean));
    return [...v].map((i) => ({ valor: i, texto: i }));
  });

  function errorDe(e) {
    error = e.mensaje;
    errores = e.datos?.errores?.length > 1 ? e.datos.errores : [];
    conflicto = e.estado === 409;
  }

  async function guardar() {
    guardando = true;
    error = '';
    errores = [];
    ok = '';
    try {
      const r = await api.put('/servidor', { revision: datos.revision, ...form });
      ok = r.reinicio ? 'Guardado. La VPN se ha reiniciado con los ajustes nuevos.' : 'Guardado sin cortar a nadie.';
      await cargar();
    } catch (e) {
      errorDe(e);
    } finally {
      guardando = false;
    }
  }

  function descartar() {
    form = JSON.parse(base);
    error = '';
    errores = [];
  }

  async function anadirRed(ev) {
    ev.preventDefault();
    anadiendo = true;
    errorRed = '';
    try {
      await api.post('/subredes', { revision: datos.revision, nombre: nuevaNombre.trim() || 'Intranet', cidr: nuevaCidr });
      nuevaNombre = '';
      nuevaCidr = '';
      await cargar();
    } catch (e) {
      errorRed = e.mensaje;
      if (e.estado === 409) await cargar();
    } finally {
      anadiendo = false;
    }
  }

  async function quitarRed(id) {
    errorRed = '';
    try {
      await api.del(`/subredes/${id}?revision=${datos.revision}`);
      await cargar();
    } catch (e) {
      errorRed = e.mensaje;
    }
  }

  async function regenerarClaves() {
    ocupado = true;
    error = '';
    try {
      await api.post('/servidor/regenerar-claves', { revision: datos.revision });
      ok = 'Claves nuevas creadas. Descarga otra vez la configuración de cada cliente.';
      confirmar = '';
      await cargar();
    } catch (e) {
      errorDe(e);
    } finally {
      ocupado = false;
    }
  }

  async function importar() {
    ocupado = true;
    error = '';
    try {
      const texto = await ficheroImportar.text();
      await api.post('/importar?reemplazar=true', texto, { cabeceras: { 'Content-Type': 'application/json' } });
      ok = 'Proyecto importado. La VPN se ha reiniciado con su configuración.';
      confirmar = '';
      ficheroImportar = null;
      await cargar();
    } catch (e) {
      errorDe(e);
    } finally {
      ocupado = false;
    }
  }

  async function copiarClave() {
    copiado = await copiar(s.clave_publica);
    setTimeout(() => (copiado = false), 1500);
  }

  const usosRed = (id) => (vivo?.clientes ?? []).filter((c) => c.toda_intranet || c.subredes.includes(id)).length;
</script>

<div class="nc-pagina">
  <Barra
    titulo="Servidor"
    subtitulo={s ? `Interfaz ${s.interfaz} · puerto ${s.puerto} · red ${s.red_vpn}` : 'Ajustes del servidor WireGuard'}
    icono="servidor"
  >
    {#snippet acciones()}
      {#if vivo}
        <Chip color={vivo.interfaz_activa ? 'verde' : 'ambar'}>{vivo.interfaz_activa ? 'VPN activa' : 'VPN parada'}</Chip>
      {/if}
    {/snippet}
  </Barra>

  {#if datos && s}
    <div class="resumen">
      <Tarjeta etiqueta="Dirección pública" icono="globo" valor="{s.endpoint}:{s.puerto}" detalle="Lo que ponen los clientes en Endpoint" />
      <Tarjeta etiqueta="Red VPN" icono="red" valor={s.red_vpn} detalle="El servidor es {s.ip_servidor}" />
      <Tarjeta
        etiqueta="Clientes"
        icono="clientes"
        valor={datos.clientes}
        detalle={vivo?.interfaz_activa ? `${conectados} conectado${conectados === 1 ? '' : 's'} ahora` : 'La interfaz no está activa'}
        color={conectados ? 'verde' : ''}
      />
    </div>

    <Pestanas
      bind:activa={pestana}
      pestanas={[
        { id: 'general', titulo: 'General' },
        { id: 'intranet', titulo: 'Intranet', contador: datos.intranet.length },
        { id: 'avanzado', titulo: 'Conexión y claves' },
      ]}
    />

    {#if error}
      <Aviso tipo="error">
        {error}
        {#if errores.length}<ul>{#each errores as x (x)}<li>{x}</li>{/each}</ul>{/if}
        {#snippet acciones()}
          {#if conflicto}<Boton tam="pequeno" icono="actualizar" onclick={cargar}>Recargar</Boton>{/if}
        {/snippet}
      </Aviso>
    {/if}
    {#if ok}<Aviso tipo="ok" cerrable oncerrar={() => (ok = '')}>{ok}</Aviso>{/if}

    <div class="contenido">
      {#if pestana === 'general'}
        <Tarjeta titulo="Conexión" subtitulo="Cómo llegan los clientes a este servidor" icono="vpn">
          <div class="formulario">
            <div class="dos ancha">
              <Campo etiqueta="Dirección pública" bind:valor={form.endpoint} placeholder="vpn.midominio.es" ayuda="IP pública o dominio de este servidor visto desde internet." requerido />
              <CampoNumero etiqueta="Puerto UDP" bind:valor={form.puerto} min={1} max={65535} ayuda="Ábrelo en el router si estás detrás de uno." requerido mas={{ texto: 'Puerto por el que escucha la VPN. Cambiarlo reinicia la VPN y hay que volver a importar la configuración en los clientes.', ejemplo: '51820', recomendado: '51820' }} />
            </div>
            <div class="dos">
              <Campo etiqueta="Red VPN" bind:valor={form.red_vpn} mono ayuda="Las IP de los clientes salen de aquí." requerido />
              <Campo etiqueta="IP del servidor en la VPN" bind:valor={form.ip_servidor} mono requerido />
            </div>
          </div>
        </Tarjeta>
        <Tarjeta titulo="Salida y nombres" subtitulo="Por dónde sale el tráfico de los clientes" icono="red">
          <div class="formulario">
            <div class="dos">
              <Campo etiqueta="Interfaz de salida" tipo="select" bind:valor={form.iface_salida} opciones={interfaces} ayuda="La que va a tu LAN o a internet." />
              <Campo etiqueta="DNS para los clientes" bind:valor={form.dns} mono placeholder="1.1.1.1, 9.9.9.9" ayuda="Solo si el cliente usa «DNS del servidor»." />
            </div>
          </div>
        </Tarjeta>
      {:else if pestana === 'intranet'}
        <Tarjeta titulo="Redes de la intranet" subtitulo="Redes de casa u oficina a las que pueden llegar los clientes" icono="casa_red">
          <div class="formulario">
            <p class="explica">
              Aquí solo se definen. A qué redes llega cada cliente se elige en su pestaña «Acceso».
            </p>
            {#if datos.intranet.length}
              <ul class="redes">
                {#each datos.intranet as r (r.id)}
                  {@const n = usosRed(r.id)}
                  <li>
                    <span class="r-icono"><Icono nombre="red" tam={16} /></span>
                    <span class="r-nombre">{r.nombre}</span>
                    <span class="mono r-cidr">{r.cidr}</span>
                    <span class="r-usos">{n} cliente{n === 1 ? '' : 's'}</span>
                    <Boton variante="fantasma" tam="pequeno" icono="papelera" titulo="Quitar {r.nombre}" onclick={() => quitarRed(r.id)} />
                  </li>
                {/each}
              </ul>
            {:else}
              <div class="sin-redes"><Icono nombre="info" tam={15} />Sin redes: los clientes solo llegan a la red VPN (y a internet si se lo permites).</div>
            {/if}
            {#if errorRed}<Aviso tipo="error">{errorRed}</Aviso>{/if}
            <form class="anadir" onsubmit={anadirRed}>
              <Campo etiqueta="Nombre" bind:valor={nuevaNombre} placeholder="Oficina" />
              <Campo etiqueta="Red" bind:valor={nuevaCidr} placeholder="192.168.10.0/24" mono />
              <Boton tipo="submit" icono="mas" cargando={anadiendo} desactivado={!nuevaCidr.trim()}>Añadir</Boton>
            </form>
          </div>
        </Tarjeta>
      {:else}
        <Tarjeta titulo="Reglas" subtitulo="Qué puede hacer el tráfico que entra por la VPN" icono="escudo">
          <div class="lista">
            <Opcion
              titulo="Permisos por cliente"
              texto="Cada cliente solo llega a lo que le permitas en «Acceso». Si lo apagas, todos llegan a todo."
              bind:activo={form.firewall}
            />
            <Opcion
              titulo="Tráfico entre clientes"
              texto="Los dispositivos de la VPN pueden verse entre sí (por ejemplo, para escritorio remoto)."
              bind:activo={form.entre_clientes}
            />
            <Opcion
              titulo="NAT (enmascarar)"
              texto={datos.firewall_modulo
                ? 'Lo gestiona el módulo Firewall, que está instalado.'
                : 'Los clientes salen con la IP de este servidor. Necesario salvo que tu router tenga una ruta a la red VPN.'}
              bind:activo={form.nat}
              desactivado={datos.firewall_modulo}
            />
          </div>
        </Tarjeta>
        <Tarjeta titulo="Interfaz" subtitulo="Cambiar esto reinicia la VPN" icono="ajustes">
          <div class="formulario">
            <div class="dos">
              <Campo etiqueta="Nombre de la interfaz" bind:valor={form.interfaz} mono />
              <CampoNumero etiqueta="Tamaño máximo de paquete (MTU)" bind:valor={form.mtu} min={1280} max={9000} placeholder="Automático" ayuda="Déjalo vacío salvo que haya problemas con webs que no cargan." mas={{ texto: 'Si algunas webs se quedan cargando por la VPN, bajarlo suele arreglarlo.', ejemplo: '1380', recomendado: 'vacío (automático)' }} />
            </div>
          </div>
        </Tarjeta>
        <Tarjeta titulo="Claves del servidor" icono="llave">
          <div class="formulario">
            <div class="clave">
              <span class="etq">Clave pública</span>
              <div class="fila">
                <code>{s.clave_publica}</code>
                <Boton variante="fantasma" tam="pequeno" icono={copiado ? 'ok' : 'copiar'} titulo="Copiar clave pública" onclick={copiarClave} />
              </div>
            </div>
            {#if confirmar === 'claves'}
              <Aviso tipo="error" titulo="¿Regenerar las claves del servidor?">
                Todos los clientes dejarán de conectar hasta que importen su configuración nueva.
                {#snippet acciones()}
                  <Boton tam="pequeno" variante="fantasma" onclick={() => (confirmar = '')}>Cancelar</Boton>
                  <Boton tam="pequeno" variante="peligro" cargando={ocupado} onclick={regenerarClaves}>Regenerar</Boton>
                {/snippet}
              </Aviso>
            {:else}
              <div class="fila-accion">
                <div><strong>Regenerar claves</strong><small>Solo si la clave privada se ha filtrado. Afecta a todos los clientes.</small></div>
                <Boton variante="peligro" icono="actualizar" onclick={() => (confirmar = 'claves')}>Regenerar</Boton>
              </div>
            {/if}
          </div>
        </Tarjeta>
        <Tarjeta titulo="Importar de wg-configurador" subtitulo="Sustituye toda la configuración por la de un proyecto .json" icono="subir">
          <div class="formulario">
            <div class="fila-accion">
              <div>
                <strong>{ficheroImportar ? ficheroImportar.name : 'Proyecto de wg-configurador'}</strong>
                <small>Se conservan las claves y los permisos de cada cliente.</small>
              </div>
              <input
                bind:this={entradaFichero}
                type="file"
                accept=".json,application/json"
                hidden
                onchange={(e) => {
                  ficheroImportar = e.currentTarget.files?.[0] ?? null;
                  if (ficheroImportar) confirmar = 'importar';
                }}
              />
              <Boton icono="subir" onclick={() => entradaFichero.click()}>Elegir fichero</Boton>
            </div>
            {#if confirmar === 'importar' && ficheroImportar}
              <Aviso tipo="aviso" titulo="¿Reemplazar la configuración actual?">
                Se sustituyen el servidor y sus {datos.clientes} clientes por los de «{ficheroImportar.name}», y la VPN se reinicia.
                {#snippet acciones()}
                  <Boton tam="pequeno" variante="fantasma" onclick={() => ((confirmar = ''), (ficheroImportar = null))}>Cancelar</Boton>
                  <Boton tam="pequeno" variante="primario" cargando={ocupado} onclick={importar}>Importar</Boton>
                {/snippet}
              </Aviso>
            {/if}
          </div>
        </Tarjeta>
      {/if}
    </div>

    {#if sucio && pestana !== 'intranet'}
      <div class="barra-guardar" class:reinicia>
        <span class="b-icono"><Icono nombre={reinicia ? 'aviso' : 'info'} tam={18} /></span>
        <div class="b-texto">
          {#if reinicia}
            <strong>Esto reinicia la VPN</strong>
            <span>
              {conectados
                ? `${conectados} cliente${conectados === 1 ? '' : 's'} conectado${conectados === 1 ? '' : 's'} se cortará${conectados === 1 ? '' : 'n'} unos segundos.`
                : 'Ahora no hay nadie conectado.'}
            </span>
          {:else}
            <strong>Cambios sin guardar</strong>
            <span>Se aplican sin cortar a nadie.</span>
          {/if}
        </div>
        <Boton variante="fantasma" onclick={descartar}>Descartar</Boton>
        <Boton variante="primario" cargando={guardando} onclick={guardar}>{reinicia ? 'Guardar y reiniciar' : 'Guardar'}</Boton>
      </div>
    {/if}
  {:else if error}
    <Aviso tipo="error" titulo="No se pudo cargar el servidor">
      {error}
      {#snippet acciones()}<Boton tam="pequeno" icono="actualizar" onclick={cargar}>Reintentar</Boton>{/snippet}
    </Aviso>
  {/if}
</div>

<style>
  .resumen {
    display: grid;
    gap: 16px;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
  .resumen :global(.m-valor) {
    font-size: 22px;
    overflow-wrap: anywhere;
  }
  .contenido {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 820px;
  }
  .formulario {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .dos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  .dos.ancha {
    grid-template-columns: 2fr 1fr;
  }
  .lista {
    display: flex;
    flex-direction: column;
    margin: -10px 0;
  }
  .explica {
    color: var(--texto-2);
    font-size: 13px;
  }
  .redes {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--borde);
    border-radius: var(--radio);
  }
  .redes li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 8px 8px 12px;
  }
  .redes li + li {
    border-top: 1px solid var(--borde-suave);
  }
  .r-icono {
    width: 30px;
    height: 30px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: var(--radio-pq);
    background: var(--azul-tinte);
    color: var(--azul);
  }
  .r-nombre {
    font-weight: 500;
    flex: 1;
    min-width: 0;
  }
  .r-cidr {
    color: var(--texto-2);
  }
  .r-usos {
    color: var(--texto-3);
    font-size: 12px;
    width: 72px;
    text-align: right;
  }
  .mono {
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .sin-redes {
    display: flex;
    gap: 8px;
    align-items: center;
    color: var(--texto-2);
    font-size: 13px;
    padding: 12px;
    border: 1px dashed var(--borde);
    border-radius: var(--radio);
  }
  .anadir {
    display: grid;
    grid-template-columns: 1fr 1fr auto;
    gap: 12px;
    align-items: end;
  }
  .clave {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .etq {
    font-size: 13px;
    color: var(--texto-2);
  }
  .fila {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .fila code {
    flex: 1;
    min-width: 0;
    padding: 7px 10px;
    overflow-wrap: anywhere;
    color: var(--texto-2);
  }
  .fila-accion {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .fila-accion div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .fila-accion strong {
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .fila-accion small {
    color: var(--texto-3);
    font-size: 12px;
  }
  .barra-guardar {
    position: sticky;
    bottom: 16px;
    z-index: 5;
    max-width: 820px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: var(--radio);
    background: var(--superficie-2);
    border: 1px solid var(--borde-fuerte);
    box-shadow: var(--sombra);
  }
  .barra-guardar.reinicia {
    border-color: color-mix(in srgb, var(--ambar) 55%, var(--borde));
  }
  .b-icono {
    display: flex;
    color: var(--azul);
  }
  .reinicia .b-icono {
    color: var(--ambar);
  }
  .b-texto {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .b-texto span {
    color: var(--texto-2);
    font-size: 12.5px;
  }
  ul {
    margin: 6px 0 0;
    padding-left: 18px;
  }
  @media (max-width: 900px) {
    .resumen {
      grid-template-columns: 1fr 1fr;
      gap: 10px;
    }
    .resumen > :global(:first-child) {
      grid-column: 1 / -1;
    }
    .resumen :global(.m-valor) {
      font-size: 18px;
    }
  }
  @media (max-width: 720px) {
    .dos,
    .dos.ancha,
    .anadir {
      grid-template-columns: 1fr;
    }
    .r-usos {
      display: none;
    }
    .barra-guardar {
      flex-wrap: wrap;
    }
    .b-texto {
      flex-basis: calc(100% - 40px);
    }
  }
</style>
