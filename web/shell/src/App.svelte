<script>
  import { onMount } from 'svelte';
  import { EVENTO_SESION_CADUCADA, Vacio, Boton, Aviso } from '@netctl/kit';
  import { sesion, cargarYo, cerrarLocal } from './sesion.svelte.js';
  import { ruta, analizar } from './ruta.svelte.js';
  import Lateral from './Lateral.svelte';
  import Cabecera from './Cabecera.svelte';
  import ModuloUI from './ModuloUI.svelte';
  import Login from './paginas/Login.svelte';
  import Inicio from './paginas/Inicio.svelte';
  import Modulos from './paginas/Modulos.svelte';
  import Ajustes from './paginas/Ajustes.svelte';

  const vista = $derived(analizar(ruta.actual));

  onMount(() => {
    cargarYo();
    const caducada = () => {
      if (sesion.yo) cerrarLocal('La sesión ha caducado. Vuelve a entrar.');
    };
    addEventListener(EVENTO_SESION_CADUCADA, caducada);
    return () => removeEventListener(EVENTO_SESION_CADUCADA, caducada);
  });

  $effect(() => {
    const m = vista.pagina === 'modulo' ? sesion.modulos.find((x) => x.id === vista.id) : null;
    const nombres = { inicio: 'Inicio', modulos: 'Módulos', ajustes: 'Ajustes' };
    const t = m ? m.nombre : nombres[vista.pagina];
    document.title = sesion.yo && t ? `${t} · netctl` : 'netctl';
  });
</script>

{#if sesion.cargando}
  <div class="arranque"><span class="giro"></span></div>
{:else if !sesion.yo}
  <Login />
{:else}
  <div class="marco">
    <Lateral {vista} />
    <div class="principal">
      <Cabecera />
      <main>
        {#if sesion.yo.password_defecto && !(vista.pagina === 'ajustes')}
          <div class="aviso-defecto">
            <Aviso tipo="aviso" titulo="Estás usando la contraseña de fábrica («root»)">
              Cualquiera que la conozca puede entrar. Cámbiala antes de seguir.
              {#snippet acciones()}<Boton variante="primario" href="/ajustes/password">Cambiar contraseña</Boton>{/snippet}
            </Aviso>
          </div>
        {/if}
        {#key vista.pagina === 'modulo' ? `m:${vista.id}` : vista.pagina}
          {#if vista.pagina === 'inicio'}
            <Inicio />
          {:else if vista.pagina === 'modulos'}
            <Modulos />
          {:else if vista.pagina === 'ajustes'}
            <Ajustes />
          {:else if vista.pagina === 'modulo'}
            <ModuloUI id={vista.id} sub={vista.sub} />
          {:else}
            <div class="nc-pagina">
              <Vacio icono="aviso" titulo="Esta página no existe" texto="Puede que el módulo se haya quitado o que el enlace esté mal.">
                {#snippet acciones()}<Boton variante="primario" href="/">Ir a Inicio</Boton>{/snippet}
              </Vacio>
            </div>
          {/if}
        {/key}
      </main>
    </div>
  </div>
{/if}

<style>
  .aviso-defecto {
    padding: 24px 28px 0;
    max-width: 1360px;
    margin: 0 auto;
  }
  @media (max-width: 720px) {
    .aviso-defecto {
      padding: 16px 14px 0;
    }
  }
  .arranque {
    height: 100vh;
    display: grid;
    place-items: center;
  }
  .giro {
    width: 26px;
    height: 26px;
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
  .marco {
    display: flex;
    min-height: 100vh;
  }
  .principal {
    flex: 1;
    min-width: 0;
    margin-left: var(--ancho-lateral);
    display: flex;
    flex-direction: column;
  }
  main {
    flex: 1;
    min-width: 0;
  }
  @media (max-width: 720px) {
    .principal {
      margin-left: 0;
      padding-bottom: 60px;
    }
  }
</style>
