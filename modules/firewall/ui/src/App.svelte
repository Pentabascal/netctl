<script>
  // Elige la página según la entrada del menú y muestra el asistente si aún no está configurado.
  // La barra de «Confirmar cambios» (60 s) vive aquí para que siga visible al cambiar de pestaña.
  import { onMount } from 'svelte';
  import { Aviso, Boton, Confirmacion60 } from '@netctl/kit';
  import { fw, aplicado, recargar, textoError } from './estado.svelte.js';
  import AsistenteInicial from './AsistenteInicial.svelte';
  import Zonas from './Zonas.svelte';
  import TablaPoliticas from './TablaPoliticas.svelte';
  import Grupos from './Grupos.svelte';
  import Registro from './Registro.svelte';

  let { api, ruta = '' } = $props();

  let config = $state(null); // respuesta de GET /config
  let error = $state('');
  let deshecho = $state(false);

  async function cargar() {
    try {
      config = await api.get('/config');
      if (config.pendiente && !fw.pendiente) fw.pendiente = { ...config.pendiente, desde: Date.now() };
      if (!config.pendiente) fw.pendiente = null;
      error = '';
    } catch (e) {
      error = textoError(e);
    }
  }

  onMount(cargar);

  let vista = 0;
  $effect(() => {
    if (fw.version !== vista) {
      vista = fw.version;
      cargar();
    }
  });

  async function confirmar() {
    const id = fw.pendiente?.id;
    await api.post(`/confirmar/${id}`);
    fw.pendiente = null;
    deshecho = false;
    recargar();
  }

  function expirado() {
    fw.pendiente = null;
    deshecho = true;
    setTimeout(recargar, 1500);
  }
</script>

<div class:con-barra={!!fw.pendiente}>
{#if error}
  <div class="nc-pagina">
    <Aviso tipo="error" titulo="No se pudo cargar el Firewall">
      {error}
      {#snippet acciones()}<Boton tam="pequeno" icono="actualizar" onclick={cargar}>Reintentar</Boton>{/snippet}
    </Aviso>
  </div>
{:else if config && !config.configurado}
  <AsistenteInicial {api} onhecho={(r) => aplicado(r)} />
{:else if config}
  {#if deshecho}
    <div class="nc-pagina aviso-arriba">
      <Aviso tipo="aviso" titulo="El último cambio se ha deshecho solo" cerrable oncerrar={() => (deshecho = false)}>
        No se confirmó en 60 segundos, así que el firewall ha vuelto a como estaba. Si perdiste el acceso, ya lo
        tienes de nuevo.
      </Aviso>
    </div>
  {/if}
  {#if ruta === 'tabla' || ruta === 'reenvios'}
    <TablaPoliticas {api} {config} />
  {:else if ruta === 'grupos'}
    <Grupos {api} {config} />
  {:else if ruta === 'registro'}
    <Registro {api} />
  {:else}
    <Zonas {api} {config} />
  {/if}
{/if}

</div>

{#if fw.pendiente}
  {#key fw.pendiente.id}
    <Confirmacion60
      segundos={fw.pendiente.segundos}
      mensaje="Cambio aplicado en el firewall. Si sigues viendo el panel, confírmalo; si no, se deshará solo."
      onconfirmar={confirmar}
      onexpirar={expirado}
    />
  {/key}
{/if}

<style>
  /* Que la barra de «Confirmar cambios» no tape el final de la página. */
  .con-barra :global(.nc-pagina) {
    padding-bottom: 120px;
  }
  .aviso-arriba {
    padding-bottom: 0;
  }
</style>
