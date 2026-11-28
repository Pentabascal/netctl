<script>
  // Política de una pareja de zonas (el lápiz de la matriz): Permitir todo / Bloquear todo.
  import { untrack } from 'svelte';
  import { PanelLateral, Bloque, Radios, Boton, Aviso } from '@netctl/kit';
  import { aplicado, textoError } from './estado.svelte.js';

  let { api, config, origen, destino, oncerrar } = $props();

  const c = $derived(config.config);
  const celda = $derived(config.matriz.find((m) => m.origen === origen && m.destino === destino));
  const nombre = (id) => c.zonas.find((z) => z.id === id)?.nombre ?? id;
  let politica = $state(untrack(() => config.matriz.find((m) => m.origen === origen && m.destino === destino)?.politica ?? 'bloquear'));
  let ocupado = $state(false);
  let error = $state('');

  const contraria = $derived(config.matriz.find((m) => m.origen === destino && m.destino === origen));

  async function guardar(p = politica) {
    ocupado = true;
    error = '';
    try {
      aplicado(await api.put(`/politicas/${origen}/${destino}`, { politica: p, revision: c.revision }));
      oncerrar();
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }
</script>

<PanelLateral abierto titulo="{nombre(origen)} → {nombre(destino)}" {oncerrar}>
  <Bloque titulo="Lo que no coincide con ninguna política" ayuda="Es lo que pasa con el tráfico de esta pareja de zonas cuando ninguna de tus políticas lo decide antes.">
    <Radios
      bind:valor={politica}
      opciones={[
        { valor: 'permitir', texto: 'Permitir todo', ayuda: 'Deja pasar todas las conexiones nuevas de esta pareja.' },
        { valor: 'bloquear', texto: 'Bloquear todo', ayuda: 'Corta las conexiones nuevas. Las respuestas a conexiones abiertas desde el otro lado siguen llegando.' },
      ]}
    />
    {#if celda && politica !== celda.de_serie}
      <span class="nota">La de serie es «{celda.de_serie === 'permitir' ? 'Permitir todo' : 'Bloquear todo'}».</span>
    {/if}
    {#if politica === 'permitir' && contraria?.politica === 'permitir' && destino !== origen}
      <span class="nota">{nombre(destino)} también puede llegar a {nombre(origen)}.</span>
    {/if}
  </Bloque>
  {#if error}<Aviso tipo="error">{error}</Aviso>{/if}
  {#snippet pie()}
    {#if celda?.cambiada}<Boton desactivado={ocupado} onclick={() => guardar(celda.de_serie)}>Volver a la de serie</Boton>{/if}
    <span class="hueco"></span>
    <Boton onclick={oncerrar}>Cancelar</Boton>
    <Boton variante="primario" cargando={ocupado} desactivado={politica === celda?.politica} onclick={() => guardar()}>Guardar</Boton>
  {/snippet}
</PanelLateral>

<style>
  .nota {
    color: var(--texto-2);
    font-size: 12px;
  }
  .hueco {
    flex: 1;
  }
</style>
