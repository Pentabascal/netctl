<script>
  // «+ Abrir puerto»: 1) a qué equipo, 2) qué servicio, 3) desde dónde (y hasta cuándo).
  // Comprueba choques y avisos en el momento; al crear enseña cómo conectarse desde fuera.
  import { onMount, untrack } from 'svelte';
  import { PanelLateral, Asistente, SelectorEquipo, CampoNumero, Campo, Aviso, Boton, Icono } from '@netctl/kit';
  import { aplicado, textoError, redesDe } from './estado.svelte.js';
  import { formNuevo, aReenvio, puertoUnico, CADUCA } from './reenvio.js';
  import ElegirServicio from './ElegirServicio.svelte';

  let { api, config, abierto = $bindable(false), equipoInicial = null } = $props();

  const c = $derived(config.config);
  const redes = $derived(redesDe(c));

  let plantillas = $state([]);
  let f = $state(formNuevo());
  let paso = $state(0);
  let valido = $state(false);
  let fueraValido = $state(true);
  let comprobado = $state(null);
  let ocupado = $state(false);
  let error = $state('');
  let hecho = $state(null); // respuesta al crear

  onMount(async () => {
    try {
      plantillas = await api.get('/plantillas');
    } catch {
      plantillas = [];
    }
  });

  $effect(() => {
    if (abierto) {
      untrack(() => {
        f = formNuevo();
        if (equipoInicial) f.equipo = equipoInicial;
        paso = 0;
        comprobado = null;
        error = '';
        hecho = null;
      });
    }
  });

  const plantilla = $derived(plantillas.find((p) => p.id === f.plantilla));
  const unico = $derived(puertoUnico(f.puertos));
  const datos = $derived(aReenvio(f, plantilla && plantilla.id !== 'otro' ? plantilla.nombre : ''));

  // Comprobar en el momento (choques con el panel, SSH, WireGuard u otros reenvíos y avisos).
  let temporizador;
  $effect(() => {
    const d = JSON.stringify(datos);
    if (paso < 1 || !f.equipo || !valido) {
      comprobado = null;
      return;
    }
    clearTimeout(temporizador);
    temporizador = setTimeout(async () => {
      try {
        comprobado = await api.post('/reenvios/comprobar', { reenvio: JSON.parse(d) });
      } catch (e) {
        comprobado = { errores: [textoError(e)], avisos: [] };
      }
    }, 250);
  });

  const errores = $derived(comprobado?.errores ?? []);
  const puedeSeguir = $derived(
    paso === 0 ? !!f.equipo : paso === 1 ? valido && fueraValido && !errores.length : !errores.length && (!f.soloAlgunas || f.desde.length > 0),
  );

  function usarLibre() {
    const libre = comprobado?.siguiente_libre;
    if (libre == null) return;
    if (unico != null) f.fuera = String(libre);
  }

  async function crear() {
    ocupado = true;
    error = '';
    try {
      const r = await api.post('/reenvios', { reenvio: datos, revision: c.revision });
      aplicado(r);
      hecho = { ...r, wan_privada: comprobado?.wan_privada };
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }

  async function copiar(t) {
    try {
      await navigator.clipboard.writeText(t);
    } catch {
      /* sin portapapeles */
    }
  }
</script>

<PanelLateral bind:abierto titulo="Abrir puerto" subtitulo="Que desde internet se llegue a un servicio de un equipo de tu red" icono="globo" ancho="640px">
  {#if hecho}
    <div class="hecho">
      <span class="ok"><Icono nombre="ok_circulo" tam={34} /></span>
      <h3>Puerto abierto</h3>
      <p>Confirma el cambio en la barra de abajo. Para conectarte desde fuera, usa:</p>
      {#each hecho.como_conectarse ?? [] as d (d)}
        <div class="dir"><code>{d}</code><Boton tam="pequeno" icono="copiar" onclick={() => copiar(d)}>Copiar</Boton></div>
      {:else}
        <p class="nota">La IP pública de esta máquina no se conoce todavía.</p>
      {/each}
      {#if hecho.wan_privada}
        <Aviso tipo="aviso" titulo="Hay otro router delante">
          La IP de internet de esta máquina es privada: también tienes que redirigir el puerto en el router de tu
          proveedor hacia esta máquina.
        </Aviso>
      {/if}
      <Boton variante="primario" onclick={() => (abierto = false)}>Hecho</Boton>
    </div>
  {:else}
    <Asistente pasos={['Equipo', 'Servicio', 'Desde dónde']} bind:paso {puedeSeguir} {ocupado} textoFin="Abrir puerto" onfin={crear}>
      {#if paso === 0}
        <p class="pregunta">¿A qué equipo de tu red tiene que llegar?</p>
        <SelectorEquipo
          etiqueta="Equipo"
          bind:valor={f.equipo}
          soloIpv4
          propias={redes.propias}
          conocidas={redes.conocidas}
          ayuda="Si lo eliges de la lista, el reenvío le sigue aunque cambie de IP."
          mas={{ texto: 'El equipo donde funciona el servicio (un servidor de juegos, una cámara, un NAS…). Los reenvíos son solo para IPv4.', ejemplo: 'NAS · 192.168.1.20' }}
        />
      {:else if paso === 1}
        <p class="pregunta">¿Qué servicio?</p>
        <ElegirServicio {plantillas} bind:plantilla={f.plantilla} bind:puertos={f.puertos} bind:protocolo={f.protocolo} bind:valido />
        {#if unico != null}
          <CampoNumero
            etiqueta="Puerto de fuera (opcional)"
            bind:valor={f.fuera}
            bind:valido={fueraValido}
            placeholder="El mismo: {unico}"
            ayuda="El que se usará desde internet. Normalmente el mismo; cámbialo si ya está usado."
            mas={{ texto: `Desde fuera se entra por este puerto y llega al ${unico} del equipo.`, ejemplo: '2222 → 22' }}
          />
        {/if}
      {:else}
        <p class="pregunta">¿Desde dónde se podrá usar?</p>
        <div class="opciones" role="radiogroup" aria-label="Desde dónde">
          <button type="button" role="radio" aria-checked={!f.soloAlgunas} class:elegida={!f.soloAlgunas} onclick={() => (f.soloAlgunas = false)}>
            <b>Desde cualquier sitio de internet{#if !plantilla?.delicado}{" "}<small class="rec">(recomendado)</small>{/if}</b>
            <small>Cualquiera que sepa tu dirección.</small>
          </button>
          <button type="button" role="radio" aria-checked={f.soloAlgunas} class:elegida={f.soloAlgunas} onclick={() => (f.soloAlgunas = true)}>
            <b>Solo desde estas IPs{#if plantilla?.delicado}{" "}<small class="rec">(recomendado)</small>{/if}</b>
            <small>Por ejemplo, la de tu oficina o la de un amigo.</small>
          </button>
        </div>
        {#if f.soloAlgunas}
          <SelectorEquipo
            bind:valor={f.desde}
            varios
            redes
            soloIpv4
            equipos={[]}
            grupos={c.grupos_ip.map((g) => ({ id: g.id, nombre: g.nombre }))}
            placeholder="Escribe una IP, red o rango de internet"
            ayuda="IPs públicas desde las que se podrá entrar."
            mas={{ ejemplo: '85.12.34.56  ·  85.12.34.0/24' }}
          />
        {/if}
        <Campo
          etiqueta="Caduca"
          tipo="select"
          bind:valor={f.caduca}
          opciones={CADUCA}
          ayuda="Útil para abrir algo un rato: al caducar se desactiva solo (no se borra)."
        />
      {/if}

      {#if paso > 0}
        {#each errores as e (e)}
          <Aviso tipo="error">
            {e}
            {#snippet acciones()}
              {#if comprobado?.siguiente_libre != null && unico != null}
                <Boton tam="pequeno" onclick={usarLibre}>Usar el {comprobado.siguiente_libre}</Boton>
              {/if}
            {/snippet}
          </Aviso>
        {/each}
        {#each comprobado?.avisos ?? [] as a (a)}
          <Aviso tipo="aviso">
            {a}
            {#snippet acciones()}
              {#if paso === 2 && !f.soloAlgunas && /peligroso/.test(a)}
                <Boton tam="pequeno" onclick={() => (f.soloAlgunas = true)}>Solo desde estas IPs</Boton>
                <a class="enlace" href="/modulo/wireguard/clientes">Ir a la VPN</a>
              {/if}
            {/snippet}
          </Aviso>
        {/each}
      {/if}
      {#if error}<Aviso tipo="error">{error}</Aviso>{/if}
    </Asistente>
  {/if}
</PanelLateral>

<style>
  .pregunta {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .opciones {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .opciones button {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 11px 14px;
    text-align: left;
    background: var(--superficie-2);
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    color: var(--texto);
    font: inherit;
    cursor: pointer;
  }
  .opciones button.elegida {
    border-color: var(--azul);
    background: var(--azul-tinte);
  }
  .opciones b {
    font-weight: 500;
  }
  small {
    color: var(--texto-3);
    font-size: 12px;
  }
  .rec {
    color: var(--verde);
  }
  .enlace {
    font-size: 12.5px;
    margin-left: 8px;
  }
  .hecho {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
    padding: 20px 8px;
  }
  .hecho p {
    margin: 0;
    color: var(--texto-2);
  }
  .ok {
    color: var(--verde);
  }
  .dir {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  code {
    font: 15px var(--mono);
    background: var(--codigo);
    padding: 6px 12px;
    border-radius: var(--radio-pq);
  }
  .nota {
    font-size: 12.5px;
  }
</style>
