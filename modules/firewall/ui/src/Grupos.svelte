<script>
  // Firewall › Grupos: listas de IPs y de puertos con nombre, para usarlas en reglas y reenvíos.
  import { onMount } from 'svelte';
  import { Barra, Pestanas, Boton, PanelLateral, Campo, CampoNumero, SelectorEquipo, Aviso, Vacio, Icono, textoDireccion } from '@netctl/kit';
  import { fw, aplicado, textoError, textoPuertos, redesDe } from './estado.svelte.js';

  let { api, config } = $props();

  let datos = $state(null);
  let pestana = $state('ip');
  let editando = $state(null); // { id?, nombre, miembros|puertos, usos }
  let textoP = $state('');
  let pValidos = $state(true);
  let ocupado = $state(false);
  let error = $state('');

  const c = $derived(config.config);
  const redes = $derived(redesDe(c));

  async function cargar() {
    try {
      datos = await api.get('/grupos');
    } catch (e) {
      error = textoError(e);
    }
  }
  onMount(cargar);
  $effect(() => {
    fw.version;
    cargar();
  });

  function abrir(g) {
    error = '';
    editando = g ? { ...g, miembros: [...(g.miembros ?? [])] } : { nombre: '', miembros: [], puertos: [], usos: [] };
    textoP = textoPuertos(editando.puertos ?? []);
  }

  async function guardar() {
    ocupado = true;
    error = '';
    const cuerpo = { tipo: pestana === 'ip' ? 'ip' : 'puertos', nombre: editando.nombre, revision: c.revision };
    if (pestana === 'ip') cuerpo.miembros = editando.miembros;
    else
      cuerpo.puertos = textoP
        .split(',')
        .map((x) => x.trim())
        .filter(Boolean)
        .map((p) => {
          const [a, b = a] = p.split('-').map(Number);
          return { desde: a, hasta: b };
        });
    try {
      aplicado(editando.id ? await api.put(`/grupos/${editando.id}`, cuerpo) : await api.post('/grupos', cuerpo));
      editando = null;
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
      aplicado(await api.del(`/grupos/${editando.id}?revision=${c.revision}`));
      editando = null;
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }

  const lista = $derived((pestana === 'ip' ? datos?.ip : datos?.puertos) ?? []);
</script>

<div class="nc-pagina">
  <Barra titulo="Grupos" subtitulo="Listas con nombre de IPs o de puertos para no repetirlas en cada regla." icono="clientes">
    {#snippet acciones()}
      <Boton variante="primario" icono="mas" onclick={() => abrir(null)}>Grupo</Boton>
    {/snippet}
  </Barra>
  <Pestanas
    pestanas={[
      { id: 'ip', titulo: 'Grupos de IPs', contador: datos?.ip?.length ?? 0 },
      { id: 'puertos', titulo: 'Grupos de puertos', contador: datos?.puertos?.length ?? 0 },
    ]}
    bind:activa={pestana}
  />
  {#if datos && !lista.length}
    <Vacio
      icono="clientes"
      titulo={pestana === 'ip' ? 'Sin grupos de IPs' : 'Sin grupos de puertos'}
      texto={pestana === 'ip' ? 'Por ejemplo «Oficina» con las IPs de tu trabajo, o «Cámaras» con sus equipos.' : 'Por ejemplo «Web» con 80 y 443.'}
    />
  {:else}
    <div class="lista">
      {#each lista as g (g.id)}
        <button type="button" class="fila" onclick={() => abrir(g)}>
          <Icono nombre={pestana === 'ip' ? 'clientes' : 'enlace'} tam={16} />
          <span class="datos">
            <b>{g.nombre}</b>
            <small>{pestana === 'ip' ? g.miembros.map((m) => textoDireccion(m)).join(', ') || 'Vacío' : textoPuertos(g.puertos)}</small>
          </span>
          <span class="usos">{g.usos.length ? `Se usa en ${g.usos.length}` : 'Sin usar'}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

{#if editando}
  <PanelLateral abierto titulo={editando.id ? editando.nombre : 'Grupo nuevo'} subtitulo={pestana === 'ip' ? 'Grupo de IPs' : 'Grupo de puertos'} icono="clientes" oncerrar={() => (editando = null)}>
    <div class="form">
      <Campo etiqueta="Nombre" bind:valor={editando.nombre} placeholder={pestana === 'ip' ? 'Oficina' : 'Web'} requerido enfocar={!editando.id} ayuda="Así aparecerá al elegirlo en una regla." />
      {#if pestana === 'ip'}
        <SelectorEquipo
          etiqueta="Miembros"
          bind:valor={editando.miembros}
          varios
          redes
          redesLista={redes.lista}
          conocidas={redes.conocidas}
          ayuda="Equipos, IPs, redes o rangos. Los equipos de la lista se siguen aunque cambien de IP."
          mas={{ ejemplo: '85.12.34.0/24  ·  192.168.1.10-192.168.1.20' }}
        />
      {:else}
        <CampoNumero etiqueta="Puertos" bind:valor={textoP} bind:valido={pValidos} puertos requerido ayuda="Separados por comas; admite rangos." mas={{ ejemplo: '80, 443, 8000-8100' }} />
      {/if}
      {#if editando.usos?.length}
        <div class="usado">
          <b>Se usa en:</b>
          <ul>{#each editando.usos as u (u)}<li>{u}</li>{/each}</ul>
        </div>
      {/if}
      {#if error}<Aviso tipo="error">{error}</Aviso>{/if}
      <div class="pie">
        {#if editando.id}
          <Boton variante="peligro" icono="papelera" desactivado={ocupado || editando.usos?.length > 0} titulo={editando.usos?.length ? 'Quítalo antes de las reglas que lo usan' : ''} onclick={borrar}>Borrar</Boton>
        {/if}
        <span class="hueco"></span>
        <Boton onclick={() => (editando = null)}>Cancelar</Boton>
        <Boton variante="primario" cargando={ocupado} desactivado={!editando.nombre.trim() || (pestana === 'puertos' && (!pValidos || !textoP.trim()))} onclick={guardar}>Guardar</Boton>
      </div>
    </div>
  </PanelLateral>
{/if}

<style>
  .lista {
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    background: var(--superficie);
  }
  .fila {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 11px 14px;
    border: 0;
    border-bottom: 1px solid var(--borde-suave);
    background: none;
    color: var(--texto-2);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .fila:last-child {
    border-bottom: 0;
  }
  .fila:hover {
    background: var(--hover);
  }
  .datos {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .datos b {
    color: var(--texto);
    font-weight: 500;
  }
  small {
    color: var(--texto-3);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .usos {
    font-size: 12.5px;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .usado {
    font-size: 13px;
    color: var(--texto-2);
  }
  .usado ul {
    margin: 4px 0 0;
    padding-left: 18px;
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
