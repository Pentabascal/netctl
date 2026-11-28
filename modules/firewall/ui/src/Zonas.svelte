<script>
  // Firewall › Zonas, como en UniFi: esquema plegable, tabla de zonas, matriz que filtra y la lista
  // de políticas debajo. Con una sola tarjeta de red se ve «Qué puede entrar a esta máquina».
  import { onMount } from 'svelte';
  import { Matriz, PanelLateral } from '@netctl/kit';
  import { fw } from './estado.svelte.js';
  import { ordenarZonas } from './listaPoliticas.js';
  import EsquemaZonas from './EsquemaZonas.svelte';
  import TablaZonas from './TablaZonas.svelte';
  import PanelZona from './PanelZona.svelte';
  import PanelPareja from './PanelPareja.svelte';
  import PanelAjustes from './PanelAjustes.svelte';
  import ListaPoliticas from './ListaPoliticas.svelte';
  import SoloEntrada from './SoloEntrada.svelte';
  import FormRegla from './FormRegla.svelte';

  let { api, config } = $props();

  const c = $derived(config.config);
  const zonas = $derived(ordenarZonas(c.zonas));
  const celdas = $derived(Object.fromEntries(config.matriz.map((m) => [`${m.origen}>${m.destino}`, m])));
  const nombreZona = (id) => c.zonas.find((z) => z.id === id)?.nombre ?? id;

  let politicas = $state(null);
  let interfaces = $state([]);
  let par = $state(null); // [origen, destino] elegido en la matriz
  let panel = $state(null); // { tipo: 'zona'|'pareja'|'ajustes'|'politica', … }

  async function cargar() {
    try {
      [politicas, interfaces] = await Promise.all([api.get('/politicas'), api.get('/interfaces').then((r) => r.interfaces)]);
    } catch {
      /* la página ya enseña el error general si la API no responde */
    }
  }
  onMount(cargar);
  $effect(() => {
    fw.version;
    cargar();
  });

  const sinZona = $derived(interfaces.filter((i) => i.nombre !== 'lo' && !c.interfaces[i.nombre]).map((i) => i.nombre));
  const todas = $derived((politicas?.propias?.length ?? 0) + (politicas?.de_serie?.length ?? 0));

  function editarRegla(id) {
    const r = c.reglas.find((x) => x.id === id);
    if (r) panel = { tipo: 'politica', regla: r, origen: r.origen_zona, destino: r.destino_zona };
  }
  function crearRegla() {
    const [o, d] = par ?? ['interna', 'externa'];
    panel = { tipo: 'politica', regla: null, origen: o, destino: d };
  }
</script>

<div class="nc-pagina">
  {#if config.una_interfaz}
    <SoloEntrada {api} {config} />
  {:else}
    <EsquemaZonas {zonas} tarjetas={config.tarjetas_de_zona ?? {}} />

    <section class="tarjeta">
      <TablaZonas
        {zonas}
        tarjetas={config.tarjetas_de_zona ?? {}}
        {sinZona}
        oneditar={(z) => (panel = { tipo: 'zona', zona: z })}
        oncrear={() => (panel = { tipo: 'zona', zona: null })}
        onajustes={() => (panel = { tipo: 'ajustes' })}
      />

      <Matriz
        {zonas}
        {todas}
        celda={(o, d) => celdas[`${o}>${d}`]}
        elegida={par ? `${par[0]}>${par[1]}` : ''}
        onfiltrar={(o, d) => (par = o ? [o, d] : null)}
        oneditar={(o, d) => (panel = { tipo: 'pareja', origen: o, destino: d })}
      />

      {#if politicas}
        <ListaPoliticas {api} {config} datos={politicas} {par} {nombreZona} oneditar={editarRegla} oncrear={crearRegla} />
      {/if}
    </section>
  {/if}
</div>

{#if panel?.tipo === 'zona'}
  <PanelZona {api} {config} zona={panel.zona} lista={interfaces} oncerrar={() => (panel = null)} />
{:else if panel?.tipo === 'pareja'}
  <PanelPareja {api} {config} origen={panel.origen} destino={panel.destino} oncerrar={() => (panel = null)} />
{:else if panel?.tipo === 'ajustes'}
  <PanelAjustes {api} {config} oncerrar={() => (panel = null)} />
{:else if panel?.tipo === 'politica'}
  <PanelLateral abierto titulo={panel.regla ? panel.regla.nombre : 'Crear política'} oncerrar={() => (panel = null)}>
    <FormRegla {api} {config} origen={panel.origen} destino={panel.destino} regla={panel.regla} onhecho={() => (panel = null)} />
  </PanelLateral>
{/if}

<style>
  .tarjeta {
    display: flex;
    flex-direction: column;
    gap: 22px;
    padding: 16px;
    background: var(--fondo);
    border: 1px solid var(--borde-suave);
    border-radius: var(--radio);
  }
</style>
