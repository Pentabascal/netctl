<script>
  // Elige la página según la entrada del menú (`ruta`) y muestra el asistente si aún no hay servidor.
  import { onMount } from 'svelte';
  import { Aviso, Boton } from '@netctl/kit';
  import Asistente from './Asistente.svelte';
  import Clientes from './Clientes.svelte';
  import Servidor from './Servidor.svelte';

  let { api, ruta = '', navegar } = $props();

  let configurado = $state(null); // null mientras carga
  let error = $state('');
  let abrir = $state(null); // cliente que se abre al entrar (tras el asistente)

  const CLAVE_ABRIR = 'netctl-wg-abrir';

  async function comprobar() {
    error = '';
    try {
      configurado = (await api.get('/servidor')).configurado;
    } catch (e) {
      error = e.mensaje;
    }
  }

  onMount(() => {
    try {
      const v = sessionStorage.getItem(CLAVE_ABRIR);
      sessionStorage.removeItem(CLAVE_ABRIR);
      if (v != null) abrir = Number(v);
    } catch {
      /* sin sessionStorage */
    }
    comprobar();
  });

  function creado(indice) {
    if (ruta === 'servidor') {
      // Se va a Clientes (la carcasa vuelve a montar el módulo) y allí se abre el cliente nuevo.
      try {
        if (indice != null) sessionStorage.setItem(CLAVE_ABRIR, String(indice));
      } catch {
        /* da igual */
      }
      navegar('clientes');
    } else {
      abrir = indice;
      configurado = true;
    }
  }
</script>

{#if error}
  <div class="nc-pagina">
    <Aviso tipo="error" titulo="No se pudo cargar WireGuard">
      {error}
      {#snippet acciones()}<Boton tam="pequeno" icono="actualizar" onclick={comprobar}>Reintentar</Boton>{/snippet}
    </Aviso>
  </div>
{:else if configurado === false}
  <Asistente {api} oncreado={creado} />
{:else if configurado}
  {#if ruta === 'servidor'}
    <Servidor {api} {navegar} />
  {:else}
    <Clientes {api} {navegar} {abrir} />
  {/if}
{/if}
