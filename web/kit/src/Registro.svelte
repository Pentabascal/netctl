<script>
  import Chip from './Chip.svelte';
  /**
   * Registro de lo que ha aplicado un módulo (§3b): fecha, quién, qué y cómo acabó.
   * entradas: [{ fecha (s UNIX), usuario, resumen, estado, detalle? }]
   * estado: aplicado | pendiente | confirmado | revertido | error
   * @type {{ entradas?: any[] }}
   */
  let { entradas = [] } = $props();
  const ESTADOS = {
    aplicado: ['azul', 'Aplicado'],
    pendiente: ['ambar', 'Esperando confirmación'],
    confirmado: ['verde', 'Confirmado'],
    revertido: ['ambar', 'Deshecho solo'],
    error: ['rojo', 'Error'],
  };
  const fecha = (s) =>
    new Date(s * 1000).toLocaleString('es-ES', { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' });
</script>

{#if !entradas.length}
  <p class="vacio">Todavía no se ha aplicado ningún cambio.</p>
{:else}
  <ul class="nc-registro">
    {#each entradas as e, i (i)}
      {@const [color, texto] = ESTADOS[e.estado] ?? ['gris', e.estado]}
      <li>
        <span class="fecha">{fecha(e.fecha)}</span>
        <div class="que">
          <span>{e.resumen}</span>
          {#if e.detalle}<code>{e.detalle}</code>{/if}
          <small>por {e.usuario}</small>
        </div>
        <Chip {color}>{texto}</Chip>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .nc-registro {
    margin: 0;
    padding: 0;
    list-style: none;
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    background: var(--superficie);
  }
  li {
    display: grid;
    grid-template-columns: 130px 1fr auto;
    gap: 14px;
    align-items: start;
    padding: 11px 14px;
    border-bottom: 1px solid var(--borde-suave);
  }
  li:last-child {
    border-bottom: 0;
  }
  .fecha {
    color: var(--texto-2);
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
  }
  .que {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  code {
    font: 11.5px var(--mono);
    color: var(--rojo);
    white-space: pre-wrap;
    word-break: break-word;
  }
  small {
    color: var(--texto-3);
    font-size: 11.5px;
  }
  .vacio {
    color: var(--texto-3);
  }
  @media (max-width: 640px) {
    li {
      grid-template-columns: 1fr;
      gap: 4px;
    }
  }
</style>
