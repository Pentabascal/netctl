<script>
  // Crear o cambiar una zona: nombre (solo las creadas por ti) y sus tarjetas de red.
  import { untrack } from 'svelte';
  import { PanelLateral, Bloque, Campo, Boton, Aviso, Chip } from '@netctl/kit';
  import { aplicado, textoError } from './estado.svelte.js';

  /** zona: null = nueva. lista: GET /interfaces. */
  let { api, config, zona = null, lista = [], oncerrar } = $props();

  const c = $derived(config.config);
  const nueva = untrack(() => !zona);
  const deSerie = untrack(() => zona && zona.tipo !== 'personalizada');
  let nombre = $state(untrack(() => zona?.nombre ?? ''));
  let elegidas = $state(untrack(() => (zona ? Object.entries(c.interfaces).filter(([, z]) => z === zona.id).map(([i]) => i) : [])));
  let ocupado = $state(false);
  let error = $state('');
  let usos = $state([]);

  const nombreZona = (id) => c.zonas.find((z) => z.id === id)?.nombre ?? id;
  const tarjetas = $derived(lista.filter((i) => i.nombre !== 'lo' && c.interfaces[i.nombre] !== 'gateway'));

  function alternar(i) {
    elegidas = elegidas.includes(i) ? elegidas.filter((x) => x !== i) : [...elegidas, i];
  }

  async function guardar() {
    ocupado = true;
    error = '';
    try {
      const cuerpo = { nombre, tarjetas: elegidas, revision: c.revision };
      aplicado(nueva ? await api.post('/zonas', cuerpo) : await api.put(`/zonas/${zona.id}`, cuerpo));
      oncerrar();
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }

  async function borrar() {
    ocupado = true;
    error = '';
    usos = [];
    try {
      aplicado(await api.del(`/zonas/${zona.id}?revision=${c.revision}`));
      oncerrar();
    } catch (e) {
      error = textoError(e);
      usos = e?.datos?.usos ?? [];
    } finally {
      ocupado = false;
    }
  }
</script>

<PanelLateral abierto titulo={nueva ? 'Crear zona' : zona.nombre} {oncerrar}>
  <Bloque>
    {#if nueva}
      <Aviso tipo="info">
        Crea zonas para separar más tus redes. Cada tarjeta solo puede estar en una zona, y una zona nueva no puede
        llegar a las demás salvo a internet y a esta máquina, hasta que tú lo cambies en la matriz.
      </Aviso>
    {/if}
    <Campo
      etiqueta="Nombre de la zona"
      bind:valor={nombre}
      desactivado={deSerie}
      placeholder="Por ejemplo: Cámaras"
      ayuda={deSerie ? 'Las zonas de serie no se pueden renombrar.' : 'Para reconocerla en la matriz y en las políticas.'}
    />
    <div class="tarjetas" role="group" aria-label="Tarjetas de red">
      <span class="etq">Tarjetas de red</span>
      {#each tarjetas as i (i.nombre)}
        {@const otra = c.interfaces[i.nombre] && c.interfaces[i.nombre] !== zona?.id}
        <label class="tarjeta">
          <input type="checkbox" checked={elegidas.includes(i.nombre)} onchange={() => alternar(i.nombre)} />
          <span class="n">{i.nombre}</span>
          <span class="ip">{(i.ips ?? []).filter((x) => !x.startsWith('fe80')).join(', ') || '—'}</span>
          {#if otra}<Chip color="gris">ahora en {nombreZona(c.interfaces[i.nombre])}</Chip>{/if}
          {#if !i.existe}<Chip color="ambar">Ya no está</Chip>{/if}
        </label>
      {:else}
        <span class="ip">No hay tarjetas de red.</span>
      {/each}
      <span class="ip">Si eliges una tarjeta que está en otra zona, se mueve a esta.</span>
    </div>
  </Bloque>
  {#if error}<Aviso tipo="error">{error}{#if usos.length}: {usos.join(', ')}{/if}</Aviso>{/if}
  {#snippet pie()}
    {#if !nueva && !deSerie}<Boton variante="peligro" icono="papelera" desactivado={ocupado} onclick={borrar}>Borrar</Boton>{/if}
    <span class="hueco"></span>
    <Boton onclick={oncerrar}>Cancelar</Boton>
    <Boton variante="primario" cargando={ocupado} desactivado={!nombre.trim()} onclick={guardar}>{nueva ? 'Crear' : 'Guardar'}</Boton>
  {/snippet}
</PanelLateral>

<style>
  .tarjetas {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .etq {
    font-size: 11.5px;
    color: var(--texto-2);
  }
  .tarjeta {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 30px;
    cursor: pointer;
  }
  .n {
    font-weight: 500;
  }
  .ip {
    color: var(--texto-2);
    font-size: 12px;
  }
  .hueco {
    flex: 1;
  }
</style>
