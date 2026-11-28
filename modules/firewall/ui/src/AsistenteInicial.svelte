<script>
  // Primera vez: 3 pasos. 1) por dónde llega internet, 2) qué es cada una de las demás
  // tarjetas, 3) revisar y aplicar (con la confirmación de 60 s).
  import { onMount } from 'svelte';
  import { Asistente, Aviso, Opcion, Chip, Icono } from '@netctl/kit';
  import { textoError } from './estado.svelte.js';

  let { api, onhecho } = $props();

  let sug = $state(null);
  let error = $state('');
  let paso = $state(0);
  let ocupado = $state(false);
  let externa = $state('');
  let zonas = $state({}); // iface → zona
  let ping = $state(true);

  onMount(async () => {
    try {
      sug = await api.get('/asistente');
      for (const i of sug.interfaces) zonas[i.nombre] = i.zona_sugerida ?? 'interna';
      externa = sug.interfaces.find((i) => i.zona_sugerida === 'externa')?.nombre ?? sug.interfaces[0]?.nombre ?? '';
    } catch (e) {
      error = textoError(e);
    }
  });

  const vpn = (n) => n.startsWith('wg');
  const fisicas = $derived((sug?.interfaces ?? []).filter((i) => !vpn(i.nombre)));
  const demas = $derived((sug?.interfaces ?? []).filter((i) => i.nombre !== externa));
  const sugerida = $derived(sug?.interfaces.find((i) => i.zona_sugerida === 'externa')?.nombre);

  const OPCIONES_ZONA = [
    { id: 'interna', nombre: 'Interna', texto: 'Tus equipos de confianza: pueden salir a internet y hablar entre ellos.' },
    { id: 'dmz', nombre: 'DMZ (zona desmilitarizada)', texto: 'Servidores expuestos a internet que no deben ver tu red interna.' },
    { id: 'vpn', nombre: 'VPN', texto: 'Interfaces de WireGuard: cada cliente tiene además sus propios permisos.' },
  ];

  $effect(() => {
    for (const i of demas) if (zonas[i.nombre] === 'externa') zonas[i.nombre] = 'interna';
  });

  const ips = (i) => i.ips.filter((x) => !x.startsWith('fe80')).map((x) => x.split('/')[0]).join(', ') || 'sin IP';

  async function aplicar() {
    ocupado = true;
    error = '';
    const interfaces = {};
    for (const i of sug.interfaces) interfaces[i.nombre] = i.nombre === externa ? 'externa' : zonas[i.nombre] === 'externa' ? 'interna' : zonas[i.nombre];
    try {
      const r = await api.post('/asistente', { interfaces, ping_externo: ping });
      onhecho?.(r);
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }
</script>

<div class="nc-pagina">
  {#if error && !sug}
    <Aviso tipo="error" titulo="No se pudieron leer las tarjetas de red">{error}</Aviso>
  {:else if sug}
    <Asistente
      titulo="Proteger esta máquina"
      frase="En tres pasos: todo lo que llegue de internet se bloquea salvo lo que permitas. El panel, SSH y la VPN siguen funcionando siempre."
      pasos={['Internet', 'Las demás redes', 'Revisar y aplicar']}
      bind:paso
      puedeSeguir={!!externa}
      {ocupado}
      textoFin="Aplicar"
      onfin={aplicar}
    >
      {#if paso === 0}
        <h3 class="pregunta">¿Por dónde llega internet?</h3>
        <p class="explica">
          Es la tarjeta conectada a tu router o a tu proveedor. Todo lo que entre por ella se bloquea salvo lo que permitas
          después.
        </p>
        <div class="opciones" role="radiogroup" aria-label="Tarjeta de internet">
          {#each fisicas as i (i.nombre)}
            <button type="button" role="radio" aria-checked={externa === i.nombre} class:elegida={externa === i.nombre} onclick={() => (externa = i.nombre)}>
              <span class="radio"></span>
              <span class="txt">
                <b>{i.nombre}{#if i.nombre === sugerida}{" "}<small class="rec">(recomendado)</small>{/if}</b>
                <small class="mono">{ips(i)}</small>
              </span>
              <Chip color={i.activa ? 'verde' : 'gris'}>{i.activa ? 'Conectada' : 'Sin cable'}</Chip>
            </button>
          {/each}
        </div>
        {#if fisicas.length === 1}
          <Aviso tipo="info">
            Esta máquina tiene una sola tarjeta de red, así que el firewall decidirá «qué puede entrar a esta máquina». No
            hay reenvío de puertos: para eso hace falta que este equipo sea el router.
          </Aviso>
        {/if}
      {:else if paso === 1}
        <h3 class="pregunta">¿Qué es cada una de las demás?</h3>
        <p class="explica">Cada tarjeta va a una zona. Lo que puede hacer cada zona lo cambias luego en la matriz.</p>
        {#each demas as i (i.nombre)}
          <div class="iface">
            <div class="cab">
              <Icono nombre={vpn(i.nombre) ? 'vpn' : 'red'} tam={16} />
              <b>{i.nombre}</b>
              <small class="mono">{ips(i)}</small>
            </div>
            <div class="zonas" role="radiogroup" aria-label="Zona de {i.nombre}">
              {#each OPCIONES_ZONA as z (z.id)}
                <button type="button" role="radio" aria-checked={zonas[i.nombre] === z.id} class:elegida={zonas[i.nombre] === z.id} onclick={() => (zonas[i.nombre] = z.id)}>
                  <span class="radio"></span>
                  <span class="txt">
                    <b>{z.nombre}{#if i.zona_sugerida === z.id}{" "}<small class="rec">(recomendado)</small>{/if}</b>
                    <small>{z.texto}</small>
                  </span>
                </button>
              {/each}
            </div>
          </div>
        {:else}
          <p class="nada">No hay más tarjetas de red: puedes pasar al siguiente paso.</p>
        {/each}
      {:else}
        <h3 class="pregunta">Revisa y aplica</h3>
        <ul class="resumen">
          <li><Icono nombre="escudo" tam={15} /> Se bloqueará todo lo que entre desde internet por <b>{externa}</b>, salvo:</li>
          {#each sug.fijas as f (f.id)}
            <li class="fija"><Icono nombre="candado" tam={13} /> <b>{f.nombre}</b> — {f.descripcion}</li>
          {/each}
          {#if fisicas.length > 1}
            <li><Icono nombre="globo" tam={15} /> Los equipos de las zonas Interna, VPN y DMZ podrán salir a internet.</li>
          {/if}
        </ul>
        <Opcion
          titulo="Responder a ping desde internet (recomendado)"
          texto="Permite comprobar desde fuera si la máquina está encendida. No abre ningún servicio."
          bind:activo={ping}
          mas={{ texto: 'El ping (ICMP) sirve para diagnosticar. Ocultarlo apenas protege: los atacantes encuentran la máquina igual.', recomendado: 'activado' }}
        />
        {#if sug.otro_firewall}
          <Aviso tipo="aviso" titulo="Se apagará {sug.otro_firewall}">
            Esta máquina usa {sug.otro_firewall}. Se desactivará y su configuración se guardará: si quitas el módulo Firewall,
            puedes volver a ella.
          </Aviso>
        {/if}
        <Aviso tipo="info">
          Al aplicar tendrás 60 segundos para confirmar. Si algo te deja sin acceso, no podrás confirmar y todo volverá a
          como estaba.
        </Aviso>
        {#if error}<Aviso tipo="error">{error}</Aviso>{/if}
      {/if}
    </Asistente>
  {/if}
</div>

<style>
  .pregunta {
    font-size: 16px;
  }
  .explica,
  .nada {
    margin: 0;
    color: var(--texto-2);
  }
  .opciones,
  .zonas {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .opciones button,
  .zonas button {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 11px 14px;
    text-align: left;
    background: var(--superficie-2);
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    color: var(--texto);
    font: inherit;
    cursor: pointer;
  }
  .opciones button:hover,
  .zonas button:hover {
    border-color: var(--borde-fuerte);
  }
  button.elegida {
    border-color: var(--azul);
    background: var(--azul-tinte);
  }
  .radio {
    width: 16px;
    height: 16px;
    flex: none;
    border-radius: 50%;
    border: 2px solid var(--borde-fuerte);
  }
  .elegida .radio {
    border-color: var(--azul);
    box-shadow: inset 0 0 0 3px var(--superficie-2);
    background: var(--azul);
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }
  .txt b {
    font-weight: 500;
  }
  small {
    color: var(--texto-3);
    font-size: 12px;
  }
  .rec {
    color: var(--verde);
    font-weight: 400;
  }
  .mono {
    font-family: var(--mono);
  }
  .iface {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--borde-suave);
  }
  .cab {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .resumen {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .resumen li {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .resumen .fija {
    padding-left: 24px;
    color: var(--texto-2);
    font-size: 13px;
  }
</style>
