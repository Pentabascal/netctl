<script>
  import Icono from './Icono.svelte';
  /**
   * Cabecera de página: migas («WireGuard › Clientes»), título y acciones a la derecha.
   * @type {{ titulo: string, migas?: {titulo: string, href?: string}[], subtitulo?: string,
   *   icono?: string, acciones?: any }}
   */
  let { titulo, migas = [], subtitulo = '', icono = '', acciones } = $props();
</script>

<div class="nc-barra">
  {#if icono}<span class="icono"><Icono nombre={icono} tam={20} /></span>{/if}
  <div class="titulos">
    {#if migas.length}
      <nav class="migas" aria-label="Ruta">
        {#each migas as m, i (i)}
          {#if m.href}<a href={m.href}>{m.titulo}</a>{:else}<span>{m.titulo}</span>{/if}
          <Icono nombre="chevron_derecha" tam={12} trazo={2} />
        {/each}
      </nav>
    {/if}
    <h1>{titulo}</h1>
    {#if subtitulo}<p>{subtitulo}</p>{/if}
  </div>
  {#if acciones}<div class="acciones">{@render acciones()}</div>{/if}
</div>

<style>
  .nc-barra {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
    min-height: 44px;
  }
  .icono {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    background: var(--azul-tinte);
    color: var(--azul);
    flex: none;
  }
  .titulos {
    flex: 1;
    min-width: 200px;
  }
  .migas {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12.5px;
    color: var(--texto-3);
    margin-bottom: 2px;
  }
  .migas a {
    color: var(--texto-2);
  }
  h1 {
    font-size: 20px;
  }
  p {
    color: var(--texto-2);
    font-size: 13px;
    margin-top: 2px;
  }
  .acciones {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
</style>
