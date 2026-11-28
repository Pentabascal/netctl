<script>
  // Primer servidor WireGuard en 3 pasos (o importar un proyecto de wg-configurador).
  import { onMount } from 'svelte';
  import { Tarjeta, Campo, Boton, Aviso, Icono, Opcion, CampoNumero } from '@netctl/kit';

  let { api, oncreado } = $props();

  const pasos = [
    { titulo: 'Servidor', texto: 'Dirección y puerto' },
    { titulo: 'Redes', texto: 'VPN e intranet' },
    { titulo: 'Primer cliente', corto: 'Cliente', texto: 'Tu dispositivo' },
  ];

  let paso = $state(0);
  let f = $state({
    endpoint: '',
    puerto: '51820',
    iface_salida: 'eth0',
    red_vpn: '10.8.0.0/24',
    dns: '1.1.1.1',
    intranet: [],
    cliente: 'Mi móvil',
    internet: false,
  });
  let interfaces = $state([]);
  let error = $state('');
  let errores = $state([]);
  let creando = $state(false);
  let intentado = $state(false);

  let fichero = $state();
  let importando = $state(false);

  onMount(async () => {
    try {
      const s = await api.get('/asistente');
      f.puerto = s.puerto;
      f.red_vpn = s.red_vpn;
      f.dns = s.dns;
      f.iface_salida = s.iface_salida;
      f.intranet = s.intranet.map((cidr, i) => ({ nombre: i === 0 ? 'LAN principal' : `Red ${i + 1}`, cidr }));
      interfaces = s.interfaces;
    } catch {
      /* se quedan los valores por defecto */
    }
  });

  const errEndpoint = $derived(intentado && !f.endpoint.trim() ? 'Hace falta para que los clientes encuentren el servidor' : '');
  const errPuerto = $derived(/^\d+$/.test(f.puerto.trim()) && +f.puerto >= 1 && +f.puerto <= 65535 ? '' : 'Un número entre 1 y 65535');
  const errRed = $derived(/^\d+\.\d+\.\d+\.\d+\/\d+$/.test(f.red_vpn.trim()) ? '' : 'Escríbela como 10.8.0.0/24');
  const opcionesIface = $derived(
    [...new Set([f.iface_salida, ...interfaces].filter(Boolean))].map((i) => ({ valor: i, texto: i })),
  );

  function siguiente() {
    if (paso === 0) {
      intentado = true;
      if (errEndpoint || errPuerto || !f.endpoint.trim()) return;
    }
    if (paso === 1 && errRed) return;
    paso++;
  }

  async function crear() {
    creando = true;
    error = '';
    errores = [];
    try {
      const r = await api.post('/asistente', {
        endpoint: f.endpoint,
        puerto: f.puerto,
        red_vpn: f.red_vpn,
        iface_salida: f.iface_salida,
        dns: f.dns,
        intranet: f.intranet.filter((r) => r.cidr.trim()),
        cliente: f.cliente.trim() ? { nombre: f.cliente, internet: f.internet } : null,
      });
      oncreado(r.indice);
    } catch (e) {
      error = e.mensaje;
      errores = e.datos?.errores?.length > 1 ? e.datos.errores : [];
    } finally {
      creando = false;
    }
  }

  async function importar(e) {
    const archivo = e.currentTarget.files?.[0];
    if (!archivo) return;
    importando = true;
    error = '';
    try {
      await api.post('/importar', await archivo.text(), { cabeceras: { 'Content-Type': 'application/json' } });
      oncreado(null);
    } catch (err) {
      error = err.mensaje;
      errores = err.datos?.errores?.length > 1 ? err.datos.errores : [];
    } finally {
      importando = false;
      e.currentTarget.value = '';
    }
  }
</script>

<div class="nc-pagina asistente">
  <header class="hero">
    <span class="hero-icono"><Icono nombre="vpn" tam={26} trazo={1.5} /></span>
    <h1>Crea tu VPN WireGuard</h1>
    <p>Tres pasos y listo. Los valores por defecto ya funcionan: normalmente solo tienes que escribir la dirección pública.</p>
  </header>

  <ol class="pasos" aria-label="Pasos">
    {#each pasos as p, i (i)}
      <li class:hecho={i < paso} class:actual={i === paso} aria-current={i === paso ? 'step' : undefined}>
        <span class="num">{#if i < paso}<Icono nombre="ok" tam={14} trazo={2.5} />{:else}{i + 1}{/if}</span>
        <span class="p-textos"><strong class="largo">{p.titulo}</strong><strong class="corto">{p.corto ?? p.titulo}</strong><small>{p.texto}</small></span>
      </li>
    {/each}
  </ol>

  <Tarjeta>
    <div class="formulario">
      {#if error}
        <Aviso tipo="error">
          {error}
          {#if errores.length}<ul>{#each errores as x (x)}<li>{x}</li>{/each}</ul>{/if}
        </Aviso>
      {/if}

      {#if paso === 0}
        <Campo
          etiqueta="Dirección pública del servidor"
          bind:valor={f.endpoint}
          placeholder="vpn.midominio.es o 203.0.113.10"
          ayuda="La IP pública o el dominio con el que se llega a esta máquina desde internet."
          mas={{ texto: 'Es la dirección que pondrán los móviles y portátiles para conectarse (Endpoint). Si tu IP pública cambia, usa un dominio de DNS dinámico.', ejemplo: 'vpn.midominio.es', recomendado: 'un dominio, si tienes uno' }}
          error={errEndpoint}
          enfocar
          requerido
        />
        <div class="dos">
          <CampoNumero
            etiqueta="Puerto UDP"
            bind:valor={f.puerto}
            min={1}
            max={65535}
            error={errPuerto}
            ayuda="Si hay un router delante, redirige este puerto a esta máquina."
            mas={{ texto: 'Puerto por el que escucha la VPN. Cualquiera libre vale; cámbialo solo si tu red bloquea el de serie.', ejemplo: '51820', recomendado: '51820' }}
          />
          <Campo
            etiqueta="Interfaz de salida"
            tipo="select"
            bind:valor={f.iface_salida}
            opciones={opcionesIface}
            ayuda="La que va a tu red o a internet (detectada sola)."
          />
        </div>
      {:else if paso === 1}
        <div class="dos">
          <Campo etiqueta="Red VPN" bind:valor={f.red_vpn} mono error={errRed} ayuda="De aquí salen las IP de los clientes." mas={{ texto: 'Una red privada que no uses en casa ni en la oficina. Caben tantos clientes como IPs tenga.', ejemplo: '10.8.0.0/24', recomendado: '10.8.0.0/24 (hasta 253 clientes)' }} />
          <Campo etiqueta="DNS para los clientes" bind:valor={f.dns} mono ayuda="Opcional. Separa varios con comas." mas={{ texto: 'Servidores que usarán los clientes para traducir nombres a IPs mientras están conectados.', ejemplo: '1.1.1.1, 9.9.9.9', recomendado: 'el DNS de tu red si tienes uno; si no, 1.1.1.1' }} />
        </div>
        <div class="intranet">
          <div class="i-cabeza">
            <h3>Intranet</h3>
            <p>Redes de casa u oficina a las que podrán llegar los clientes. Hemos detectado las de esta máquina.</p>
          </div>
          {#each f.intranet as r, i (i)}
            <div class="i-fila">
              <Campo bind:valor={r.nombre} placeholder="Nombre" />
              <Campo bind:valor={r.cidr} placeholder="192.168.1.0/24" mono />
              <Boton variante="fantasma" icono="papelera" titulo="Quitar" onclick={() => f.intranet.splice(i, 1)} />
            </div>
          {:else}
            <p class="vacia">Sin intranet: los clientes solo verán la red VPN (e internet si se lo permites).</p>
          {/each}
          <div>
            <Boton variante="fantasma" icono="mas" tam="pequeno" onclick={() => f.intranet.push({ nombre: f.intranet.length ? `Red ${f.intranet.length + 1}` : 'LAN principal', cidr: '' })}>Añadir red</Boton>
          </div>
        </div>
      {:else}
        <Campo
          etiqueta="Nombre del primer cliente"
          bind:valor={f.cliente}
          placeholder="Mi móvil"
          ayuda="Al terminar verás su código QR para escanearlo con la app WireGuard. Puedes dejarlo vacío y crearlo luego."
          enfocar
        />
        <div class="bloque">
          <Opcion
            titulo="Internet a través de la VPN"
            texto="Todo su tráfico sale por este servidor. Si lo dejas apagado, solo usa la VPN para llegar a tu intranet."
            bind:activo={f.internet}
          />
        </div>
        <div class="repaso">
          <h3 class="nc-seccion">Se va a crear</h3>
          <dl>
            <dt>Servidor</dt>
            <dd class="mono">{f.endpoint}:{f.puerto}</dd>
            <dt>Red VPN</dt>
            <dd class="mono">{f.red_vpn}</dd>
            <dt>Intranet</dt>
            <dd>{f.intranet.filter((r) => r.cidr.trim()).map((r) => `${r.nombre} (${r.cidr})`).join(', ') || 'Ninguna'}</dd>
            <dt>Salida</dt>
            <dd class="mono">{f.iface_salida}</dd>
          </dl>
        </div>
      {/if}
    </div>

    {#snippet pie()}
      <div class="pie">
        {#if paso > 0}
          <Boton variante="fantasma" icono="chevron_izquierda" onclick={() => paso--}>Atrás</Boton>
        {/if}
        <span class="nc-separador"></span>
        <span class="contador">Paso {paso + 1} de {pasos.length}</span>
        {#if paso < pasos.length - 1}
          <Boton variante="primario" onclick={siguiente}>Siguiente</Boton>
        {:else}
          <Boton variante="primario" icono="ok" cargando={creando} onclick={crear}>Crear VPN</Boton>
        {/if}
      </div>
    {/snippet}
  </Tarjeta>

  <div class="importar">
    <Icono nombre="subir" tam={16} />
    <span>¿Ya tenías un proyecto de wg-configurador?</span>
    <label class="enlace">
      {importando ? 'Importando…' : 'Importarlo'}
      <input type="file" accept=".json,application/json" hidden onchange={importar} disabled={importando} />
    </label>
  </div>
</div>

<style>
  .asistente {
    max-width: 720px;
  }
  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 8px;
    padding-top: 12px;
  }
  .hero-icono {
    width: 56px;
    height: 56px;
    border-radius: 14px;
    display: grid;
    place-items: center;
    background: var(--azul-tinte);
    color: var(--azul);
    margin-bottom: 6px;
  }
  .hero h1 {
    font-size: 22px;
  }
  .hero p {
    color: var(--texto-2);
    max-width: 520px;
  }
  .pasos {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
  }
  .pasos li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--radio);
    border: 1px solid var(--borde-suave);
    color: var(--texto-3);
  }
  .pasos li.actual {
    border-color: var(--azul);
    background: color-mix(in srgb, var(--azul-tinte) 35%, transparent);
    color: var(--texto);
  }
  .pasos li.hecho {
    color: var(--texto-2);
  }
  .num {
    width: 26px;
    height: 26px;
    flex: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 12.5px;
    font-weight: 700;
    background: var(--superficie-2);
    border: 1px solid var(--borde);
  }
  .actual .num {
    background: var(--azul);
    border-color: var(--azul);
    color: #fff;
  }
  .hecho .num {
    background: var(--verde-tinte);
    border-color: transparent;
    color: var(--verde);
  }
  .p-textos {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .p-textos strong {
    font-weight: 600;
    font-size: 13.5px;
  }
  .p-textos small {
    font-size: 12px;
    color: var(--texto-3);
  }
  .corto {
    display: none;
  }
  .formulario {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .dos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  .intranet {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .i-cabeza p {
    color: var(--texto-2);
    font-size: 13px;
    margin-top: 2px;
  }
  .i-fila {
    display: grid;
    grid-template-columns: 1fr 1fr auto;
    gap: 10px;
    align-items: start;
  }
  .vacia {
    color: var(--texto-3);
    font-size: 13px;
  }
  .bloque {
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    padding: 2px 14px;
  }
  .repaso dl {
    display: grid;
    grid-template-columns: 110px 1fr;
    gap: 8px 12px;
    margin: 0;
    padding: 14px;
    border-radius: var(--radio);
    background: var(--campo);
    border: 1px solid var(--borde-suave);
  }
  dt {
    color: var(--texto-3);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .pie {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
  }
  .contador {
    color: var(--texto-3);
    font-size: 12.5px;
  }
  .importar {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    color: var(--texto-2);
    font-size: 13px;
    flex-wrap: wrap;
  }
  .enlace {
    color: var(--azul);
    cursor: pointer;
  }
  .enlace:hover {
    text-decoration: underline;
  }
  ul {
    margin: 6px 0 0;
    padding-left: 18px;
  }
  @media (max-width: 720px) {
    .pasos {
      grid-template-columns: repeat(3, auto);
      justify-content: space-between;
    }
    .p-textos small,
    .largo {
      display: none;
    }
    .p-textos .corto {
      display: block;
    }
    .pasos li {
      padding: 8px;
      border: 0;
      background: none !important;
    }
    .dos {
      grid-template-columns: 1fr;
    }
    .i-fila {
      grid-template-columns: 1fr 1fr auto;
    }
    .contador {
      display: none;
    }
  }
</style>
