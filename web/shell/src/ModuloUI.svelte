<script>
  // Monta la interfaz de un módulo: `import('/m/<id>/ui/index.js')` y `mount(el, api, ctx)`.
  import { crearApi, Aviso, Vacio, Boton } from '@netctl/kit';
  import { sesion } from './sesion.svelte.js';
  import { navegar } from './ruta.svelte.js';

  let { id, sub = '', base = '' } = $props();
  let el = $state();
  let error = $state('');
  let cargando = $state(true);

  const modulo = $derived(sesion.modulos.find((m) => m.id === id));

  $effect(() => {
    if (!el || !modulo) return;
    const ruta = sub;
    let desmontar = null;
    let vivo = true;
    error = '';
    cargando = true;
    import(/* @vite-ignore */ `/m/${id}/ui/index.js?v=${encodeURIComponent(modulo.version)}`)
      .then((m) => {
        if (!vivo) return;
        if (typeof m.mount !== 'function') throw new Error('La interfaz del módulo no exporta mount()');
        const api = crearApi(`/api/m/${id}`);
        desmontar = m.mount(el, api, {
          modulo: id,
          ruta,
          navegar: (r) => navegar(`${base || `/modulo/${id}`}${r ? '/' + String(r).replace(/^\/+/, '') : ''}`),
        });
      })
      .catch((e) => {
        if (vivo) error = e?.message || String(e);
      })
      .finally(() => {
        if (vivo) cargando = false;
      });
    return () => {
      vivo = false;
      try {
        desmontar?.();
      } catch {
        /* el módulo falló al desmontar */
      }
      el?.replaceChildren();
    };
  });
</script>

{#if !modulo}
  <div class="nc-pagina">
    <Vacio icono="modulos" titulo="Este módulo no está instalado" texto="Puedes instalarlo desde la página Módulos.">
      {#snippet acciones()}<Boton variante="primario" href="/modulos">Ver módulos</Boton>{/snippet}
    </Vacio>
  </div>
{:else}
  {#if error}
    <div class="nc-pagina">
      <Aviso tipo="error" titulo="No se pudo cargar la interfaz de {modulo.nombre}">
        {error}. Comprueba en Módulos que está en marcha.
      </Aviso>
    </div>
  {:else if cargando}
    <div class="cargando"><span class="giro"></span></div>
  {/if}
  <div class="anfitrion" bind:this={el}></div>
{/if}

<style>
  .cargando {
    display: grid;
    place-items: center;
    height: 200px;
  }
  .giro {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 2.5px solid var(--borde-fuerte);
    border-top-color: var(--azul);
    animation: girar 0.8s linear infinite;
  }
  @keyframes girar {
    to {
      transform: rotate(360deg);
    }
  }
</style>
