<script>
  /**
   * pestanas: [{ id, titulo, contador? }]
   * @type {{ pestanas: {id: string, titulo: string, contador?: number}[], activa?: string,
   *   onchange?: (id: string) => void }}
   */
  let { pestanas, activa = $bindable(pestanas[0]?.id), onchange } = $props();
  function elegir(id) {
    activa = id;
    onchange?.(id);
  }
</script>

<div class="nc-pestanas" role="tablist">
  {#each pestanas as p (p.id)}
    <button
      type="button"
      role="tab"
      aria-selected={activa === p.id}
      class:activa={activa === p.id}
      onclick={() => elegir(p.id)}
    >
      {p.titulo}
      {#if p.contador !== undefined}<span class="contador">{p.contador}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .nc-pestanas {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--borde);
    overflow-x: auto;
    scrollbar-width: none;
  }
  .nc-pestanas::-webkit-scrollbar {
    display: none;
  }
  button {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    background: none;
    border: 0;
    padding: 10px 12px 12px;
    font: inherit;
    font-weight: 500;
    color: var(--texto-2);
    cursor: pointer;
    white-space: nowrap;
  }
  button:hover {
    color: var(--texto);
  }
  .activa {
    color: var(--texto);
  }
  .activa::after {
    content: '';
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: -1px;
    height: 2px;
    border-radius: 2px;
    background: var(--azul);
  }
  .contador {
    font-size: 11.5px;
    min-width: 20px;
    height: 18px;
    padding: 0 6px;
    border-radius: 9px;
    display: inline-grid;
    place-items: center;
    background: var(--hover);
    color: var(--texto-2);
  }
  .activa .contador {
    background: var(--azul-tinte);
    color: var(--azul);
  }
</style>
