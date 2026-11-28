<script>
  // Pestaña «Tarjetas y zonas»: a qué zona va cada tarjeta y las zonas creadas por el usuario.
  import { onMount } from 'svelte';
  import { Chip, Campo, Boton, Aviso, Icono } from '@netctl/kit';
  import { aplicado, textoError, NOMBRE_ZONA_TIPO, fw } from './estado.svelte.js';

  let { api, config } = $props();

  let lista = $state([]);
  let error = $state('');
  let nuevaZona = $state('');
  let ocupado = $state(false);

  const c = $derived(config.config);
  const elegibles = $derived(c.zonas.filter((z) => z.tipo !== 'gateway'));

  async function cargar() {
    try {
      lista = (await api.get('/interfaces')).interfaces;
    } catch (e) {
      error = textoError(e);
    }
  }
  onMount(cargar);
  $effect(() => {
    fw.version;
    cargar();
  });

  async function asignar(i, zona) {
    error = '';
    try {
      aplicado(await api.put(`/interfaces/${encodeURIComponent(i)}`, { zona, revision: c.revision }));
    } catch (e) {
      error = textoError(e);
    }
  }

  /** Las zonas se guardan con PUT /config (toda la configuración). */
  async function guardarZonas(zonas) {
    ocupado = true;
    error = '';
    try {
      aplicado(await api.put('/config', { config: { ...c, zonas } }));
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }

  function crearZona() {
    const nombre = nuevaZona.trim();
    if (!nombre) return;
    let base =
      nombre
        .normalize('NFD')
        .replace(/[̀-ͯ]/g, '')
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, '_')
        .replace(/^_|_$/g, '')
        .slice(0, 16) || 'zona';
    let id = base;
    for (let n = 2; c.zonas.some((z) => z.id === id); n++) id = `${base}_${n}`;
    nuevaZona = '';
    guardarZonas([...c.zonas, { id, nombre, tipo: 'personalizada' }]);
  }

  function renombrar(z, nombre) {
    if (!nombre.trim() || nombre === z.nombre) return;
    guardarZonas(c.zonas.map((x) => (x.id === z.id ? { ...x, nombre: nombre.trim() } : x)));
  }

  const usada = (z) => Object.values(c.interfaces).includes(z.id) || c.reglas.some((r) => r.origen_zona === z.id || r.destino_zona === z.id);
  const ips = (i) => (i.ips ?? []).filter((x) => !x.startsWith('fe80')).join(', ') || '—';
</script>

<p class="frase">
  Cada tarjeta de red pertenece a una zona. Si conectas una tarjeta nueva (un USB, una VLAN), aparece aquí sin zona y
  todo lo que entre por ella se bloquea hasta que se la asignes.
</p>

<div class="tabla">
  {#each lista as i (i.nombre)}
    <div class="fila">
      <span class="ico"><Icono nombre={i.nombre.startsWith('wg') ? 'vpn' : 'red'} tam={17} /></span>
      <div class="datos">
        <b>{i.nombre}</b>
        <small class="mono">{ips(i)}{#if i.mac}{' · '}{i.mac}{/if}</small>
      </div>
      <Chip color={!i.existe ? 'ambar' : i.activa ? 'verde' : 'gris'}>{!i.existe ? 'Ya no está' : i.activa ? 'Conectada' : 'Sin cable'}</Chip>
      <div class="zona">
        <Campo
          tipo="select"
          valor={i.zona ?? ''}
          opciones={[...(i.zona ? [] : [{ valor: '', texto: 'Sin zona (bloqueada)' }]), ...elegibles.map((z) => ({ valor: z.id, texto: z.nombre }))]}
          oninput={(e) => asignar(i.nombre, e.target.value)}
        />
      </div>
    </div>
  {/each}
</div>
{#if lista.some((i) => !i.zona)}
  <Aviso tipo="aviso">Hay tarjetas sin zona: todo lo que entre por ellas se bloquea. Elige su zona arriba.</Aviso>
{/if}

<h3 class="nc-seccion">Zonas</h3>
<p class="frase">Las de serie no se pueden borrar. Crea zonas propias para separar, por ejemplo, invitados o cámaras.</p>
<div class="tabla">
  {#each c.zonas as z (z.id)}
    <div class="fila">
      <span class="ico"><Icono nombre={z.tipo === 'gateway' ? 'servidor' : 'escudo'} tam={17} /></span>
      <div class="datos">
        {#if z.tipo === 'personalizada'}
          <input class="renombrar" value={z.nombre} aria-label="Nombre de la zona" onchange={(e) => renombrar(z, e.target.value)} />
        {:else}
          <b>{z.nombre}</b>
        {/if}
        <small>{NOMBRE_ZONA_TIPO[z.tipo]}</small>
      </div>
      <span class="cuenta">{Object.values(c.interfaces).filter((x) => x === z.id).length} tarjetas</span>
      {#if z.tipo === 'personalizada'}
        <Boton
          variante="fantasma"
          icono="papelera"
          tam="pequeno"
          desactivado={usada(z) || ocupado}
          titulo={usada(z) ? 'Tiene tarjetas o reglas: quítalas antes de borrarla' : 'Borrar zona'}
          onclick={() => guardarZonas(c.zonas.filter((x) => x.id !== z.id))}
        />
      {:else}
        <span class="candado" title="Zona de serie"><Icono nombre="candado" tam={14} /></span>
      {/if}
    </div>
  {/each}
</div>
<div class="nueva">
  <Campo bind:valor={nuevaZona} placeholder="Invitados" etiqueta="Zona nueva" ayuda="Nace bloqueada hacia tus otras zonas y con salida a internet." />
  <Boton icono="mas" desactivado={!nuevaZona.trim() || ocupado} onclick={crearZona}>Crear zona</Boton>
</div>
{#if error}<Aviso tipo="error">{error}</Aviso>{/if}

<style>
  .frase {
    margin: 0;
    color: var(--texto-2);
  }
  .tabla {
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    background: var(--superficie);
  }
  .fila {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--borde-suave);
  }
  .fila:last-child {
    border-bottom: 0;
  }
  .ico,
  .candado {
    color: var(--texto-2);
    display: inline-flex;
  }
  .datos {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .datos b {
    font-weight: 500;
  }
  small {
    color: var(--texto-3);
    font-size: 12px;
  }
  .mono {
    font-family: var(--mono);
  }
  .zona {
    width: 220px;
  }
  .cuenta {
    color: var(--texto-3);
    font-size: 12.5px;
  }
  .renombrar {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 4px;
    color: var(--texto);
    font: 500 14px var(--fuente);
    padding: 2px 4px;
    margin-left: -5px;
  }
  .renombrar:hover,
  .renombrar:focus {
    border-color: var(--borde-fuerte);
    outline: none;
  }
  .nueva {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    max-width: 520px;
  }
  .nueva :global(.nc-campo) {
    flex: 1;
  }
  .nueva :global(button) {
    margin-top: 23px;
  }
</style>
