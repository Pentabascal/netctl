<script>
  // Fila de ajuste con título, explicación y un interruptor a la derecha (estilo UniFi).
  import Interruptor from './Interruptor.svelte';
  import Ayuda from './Ayuda.svelte';
  /** @type {{ titulo: string, texto?: string, activo?: boolean, desactivado?: boolean,
   *   onchange?: (v: boolean) => void, children?: any, mas?: any }} */
  let { titulo, texto = '', activo = $bindable(false), desactivado = false, onchange, children, mas } = $props();
</script>

<div class="opcion" class:apagada={desactivado}>
  <div class="textos">
    <span class="titulo"
      >{titulo}{#if mas}<Ayuda texto={mas.texto} ejemplo={mas.ejemplo} recomendado={mas.recomendado} />{/if}</span
    >
    {#if texto}<span class="texto">{texto}</span>{/if}
    {@render children?.()}
  </div>
  <Interruptor bind:activo {desactivado} etiqueta={titulo} {onchange} />
</div>

<style>
  .opcion {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 0;
  }
  .opcion + :global(.opcion) {
    border-top: 1px solid var(--borde-suave);
  }
  .textos {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .titulo {
    font-weight: 500;
  }
  .texto {
    color: var(--texto-2);
    font-size: 12.5px;
    line-height: 1.5;
  }
  .apagada .titulo {
    color: var(--texto-2);
  }
</style>
