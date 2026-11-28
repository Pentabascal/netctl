<script>
  // Una sola tarjeta de red: «Qué puede entrar a esta máquina». Lista de lo abierto (fijo con
  // candado + lo tuyo) y «Abrir un puerto» en 2 pasos: servicio → desde dónde.
  import { onMount } from 'svelte';
  import { Boton, Aviso, Icono, Interruptor, PanelLateral, Asistente, SelectorEquipo, textoDireccion } from '@netctl/kit';
  import { aplicado, textoError, textoPuertos, textoProto, redesDe } from './estado.svelte.js';
  import ElegirServicio from './ElegirServicio.svelte';
  import FormPolitica from './FormPolitica.svelte';

  let { api, config } = $props();

  const c = $derived(config.config);
  const externa = $derived(c.zonas.find((z) => z.tipo === 'externa')?.id ?? 'externa');
  const gateway = $derived(c.zonas.find((z) => z.tipo === 'gateway')?.id ?? 'gateway');
  const reglas = $derived(c.reglas.filter((r) => r.origen_zona === externa && r.destino_zona === gateway));
  const redes = $derived(redesDe(c));
  const nombresGrupos = $derived(Object.fromEntries(c.grupos_ip.map((g) => [g.id, g.nombre])));

  let plantillas = $state([]);
  let abrir = $state(false);
  let editar = $state(null);
  let paso = $state(0);
  let plantilla = $state('');
  let puertos = $state('');
  let protocolo = $state('tcp');
  let valido = $state(false);
  let soloAlgunas = $state(false);
  let desde = $state([]);
  let ocupado = $state(false);
  let error = $state('');

  onMount(async () => {
    try {
      plantillas = await api.get('/plantillas');
    } catch {
      plantillas = [];
    }
  });

  const elegida = $derived(plantillas.find((p) => p.id === plantilla));

  function empezar() {
    paso = 0;
    plantilla = '';
    puertos = '';
    soloAlgunas = false;
    desde = [];
    error = '';
    abrir = true;
  }

  async function crear() {
    ocupado = true;
    error = '';
    const valor = puertos
      .split(',')
      .map((x) => x.trim())
      .filter(Boolean)
      .map((p) => {
        const [a, b = a] = p.split('-').map(Number);
        return { desde: a, hasta: b };
      });
    const regla = {
      nombre: elegida && plantilla !== 'otro' ? elegida.nombre : `Puerto ${puertos}`,
      origen_zona: externa,
      destino_zona: gateway,
      accion: 'permitir',
      protocolo,
      puertos: { tipo: 'lista', valor },
      origen: soloAlgunas && desde.length === 1 ? desde[0] : { tipo: 'cualquiera' },
      destino: { tipo: 'cualquiera' },
    };
    try {
      if (soloAlgunas && desde.length > 1) {
        // Varias IPs: se guardan como grupo para no repetir la regla.
        const g = await api.post('/grupos', { tipo: 'ip', nombre: `Acceso a ${regla.nombre}`, miembros: desde, revision: c.revision });
        aplicado(g);
        regla.origen = { tipo: 'grupo', id: g.id };
        aplicado(await api.post('/reglas', { regla, revision: g.revision }));
      } else {
        aplicado(await api.post('/reglas', { regla, revision: c.revision }));
      }
      abrir = false;
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }

  async function activar(r, activa) {
    try {
      aplicado(await api.put(`/reglas/${r.id}`, { regla: { ...r, activa }, revision: c.revision }));
    } catch (e) {
      error = textoError(e);
    }
  }

  const esPrivada = (ip) => /^(10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.|100\.(6[4-9]|[7-9]\d|1[01]\d|12[0-7])\.)/.test(ip);
  const ipPrivada = $derived((config.ips_externas ?? []).find(esPrivada) ?? '');
  const delicado = $derived(!soloAlgunas && elegida?.delicado);
</script>

<div class="cab">
  <p class="frase">
    Todo lo que llegue a esta máquina desde otros equipos se bloquea, salvo lo de esta lista. Las conexiones que la
    propia máquina inicia siempre funcionan.
  </p>
  <Boton variante="primario" icono="mas" onclick={empezar}>Abrir un puerto</Boton>
</div>

<div class="lista">
  {#each config.fijas as f (f.id)}
    <div class="fila fija">
      <Icono nombre="candado" tam={15} />
      <div class="datos"><b>{f.nombre}</b><small>{f.descripcion}</small></div>
      <span class="nota">Siempre abierto</span>
    </div>
  {/each}
  {#each reglas as r (r.id)}
    <div class="fila" class:apagada={!r.activa}>
      <Icono nombre={r.accion === 'permitir' ? 'ok_circulo' : 'escudo'} tam={15} />
      <button type="button" class="datos" onclick={() => (editar = r)}>
        <b>{r.nombre}</b>
        <small>
          {r.accion === 'permitir' ? 'Abierto' : 'Cerrado'} · {textoProto(r.protocolo)}{#if r.puertos?.tipo === 'lista'}{' '}{textoPuertos(r.puertos.valor)}{/if}
          · desde {textoDireccion(r.origen, nombresGrupos).toLowerCase() === 'cualquiera' ? 'cualquier sitio' : textoDireccion(r.origen, nombresGrupos)}
        </small>
      </button>
      <Interruptor activo={r.activa} etiqueta="Activa" onchange={(v) => activar(r, v)} />
    </div>
  {/each}
</div>

{#if ipPrivada}
  <Aviso tipo="info" titulo="¿Quieres llegar a esta máquina desde internet?">
    Esta máquina está detrás de otro router (su IP, {ipPrivada}, es privada). Además de abrir el puerto aquí, ábrelo
    (redirígelo) en ese router hacia {ipPrivada}. Otra opción más segura: entra por la VPN.
  </Aviso>
{/if}
{#if error && !abrir}<Aviso tipo="error">{error}</Aviso>{/if}

<PanelLateral bind:abierto={abrir} titulo="Abrir un puerto" subtitulo="Para usar un servicio de esta máquina desde otros equipos" icono="escudo" ancho="600px">
  <Asistente pasos={['Servicio', 'Desde dónde']} bind:paso puedeSeguir={paso === 0 ? valido : true} {ocupado} textoFin="Abrir puerto" onfin={crear}>
    {#if paso === 0}
      <p class="frase">¿Qué servicio quieres usar desde fuera?</p>
      <ElegirServicio {plantillas} bind:plantilla bind:puertos bind:protocolo bind:valido />
    {:else}
      <p class="frase">¿Desde dónde se podrá usar {elegida?.nombre ?? `el puerto ${puertos}`}?</p>
      <div class="opciones" role="radiogroup" aria-label="Desde dónde">
        <button type="button" role="radio" aria-checked={!soloAlgunas} class:elegida={!soloAlgunas} onclick={() => (soloAlgunas = false)}>
          <b>Desde cualquier sitio{#if !elegida?.delicado}{" "}<small class="rec">(recomendado)</small>{/if}</b>
          <small>Cualquier equipo que llegue a esta máquina.</small>
        </button>
        <button type="button" role="radio" aria-checked={soloAlgunas} class:elegida={soloAlgunas} onclick={() => (soloAlgunas = true)}>
          <b>Solo desde estas IPs{#if elegida?.delicado}{" "}<small class="rec">(recomendado)</small>{/if}</b>
          <small>Equipos, IPs, redes o rangos concretos.</small>
        </button>
      </div>
      {#if soloAlgunas}
        <SelectorEquipo bind:valor={desde} varios redes redesLista={redes.lista} conocidas={redes.conocidas} grupos={c.grupos_ip.map((g) => ({ id: g.id, nombre: g.nombre }))} />
      {/if}
      {#if delicado}
        <Aviso tipo="aviso" titulo="{elegida.nombre} abierto a todos es peligroso">
          Lo atacan robots a diario. Mejor elige «Solo desde estas IPs» o entra por la VPN.
          {#snippet acciones()}<Boton tam="pequeno" onclick={() => (soloAlgunas = true)}>Solo desde estas IPs</Boton>{/snippet}
        </Aviso>
      {/if}
      {#if error}<Aviso tipo="error">{error}</Aviso>{/if}
    {/if}
  </Asistente>
</PanelLateral>

{#if editar}
  <FormPolitica {api} {config} origen={externa} destino={gateway} regla={editar} oncerrar={() => (editar = null)} />
{/if}

<style>
  .cab {
    display: flex;
    align-items: center;
    gap: 16px;
    justify-content: space-between;
  }
  .frase {
    margin: 0;
    color: var(--texto-2);
  }
  .lista {
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    background: var(--superficie);
  }
  .fila {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 11px 14px;
    border-bottom: 1px solid var(--borde-suave);
    color: var(--verde);
  }
  .fila:last-child {
    border-bottom: 0;
  }
  .fila.fija {
    color: var(--texto-3);
  }
  .fila.apagada .datos {
    opacity: 0.5;
  }
  .datos {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    border: 0;
    background: none;
    color: var(--texto);
    font: inherit;
    text-align: left;
    padding: 0;
  }
  button.datos {
    cursor: pointer;
  }
  .datos b {
    font-weight: 500;
  }
  small {
    color: var(--texto-3);
    font-size: 12px;
  }
  .nota {
    color: var(--texto-3);
    font-size: 12px;
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
  .rec {
    color: var(--verde);
    font-weight: 400;
  }
</style>
