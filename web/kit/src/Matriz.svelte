<script>
  /**
   * Matriz zona→zona del firewall: filas = desde dónde, columnas = hacia dónde. Cada casilla
   * en verde (Permitir) o rojo (Bloquear) con el número de reglas propias.
   * `celda(o, d)` → { politica: 'permitir'|'bloquear', reglas: number, cambiada?: boolean }
   * @type {{ zonas: {id: string, nombre: string}[], celda: (o: string, d: string) => any,
   *   elegida?: string, onelegir?: (o: string, d: string) => void }}
   */
  let { zonas, celda, elegida = '', onelegir } = $props();
</script>

<div class="nc-matriz" role="grid" aria-label="Qué zona puede hablar con cuál">
  <table>
    <thead>
      <tr>
        <th class="esquina"><span>Desde ↓</span><span>Hacia →</span></th>
        {#each zonas as d (d.id)}<th scope="col">{d.nombre}</th>{/each}
      </tr>
    </thead>
    <tbody>
      {#each zonas as o (o.id)}
        <tr>
          <th scope="row">{o.nombre}</th>
          {#each zonas as d (d.id)}
            {@const c = celda(o.id, d.id)}
            <td>
              {#if c}
                <button
                  type="button"
                  class="casilla {c.politica}"
                  class:elegida={elegida === `${o.id}>${d.id}`}
                  aria-label="{o.nombre} hacia {d.nombre}: {c.politica === 'permitir' ? 'permitido' : 'bloqueado'}, {c.reglas} reglas"
                  onclick={() => onelegir?.(o.id, d.id)}
                >
                  <span class="pol">{c.politica === 'permitir' ? 'Permitir' : 'Bloquear'}</span>
                  {#if c.reglas}<span class="num" title="Reglas propias">{c.reglas}</span>{/if}
                  {#if c.cambiada}<span class="marca" title="Cambiada respecto a la de serie">•</span>{/if}
                </button>
              {:else}
                <span class="nada" aria-hidden="true">—</span>
              {/if}
            </td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .nc-matriz {
    overflow-x: auto;
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    background: var(--superficie);
  }
  table {
    border-collapse: collapse;
    width: 100%;
    min-width: 560px;
  }
  th,
  td {
    padding: 6px;
    text-align: center;
    border-bottom: 1px solid var(--borde-suave);
  }
  thead th {
    font-size: 12px;
    font-weight: 600;
    color: var(--texto-2);
    padding: 10px 6px;
  }
  tbody th {
    text-align: left;
    padding-left: 14px;
    font-weight: 500;
    white-space: nowrap;
  }
  .esquina {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    font-size: 11px;
    color: var(--texto-3);
    font-weight: 500;
    padding-left: 14px;
  }
  tr:last-child th,
  tr:last-child td {
    border-bottom: 0;
  }
  .casilla {
    position: relative;
    width: 100%;
    min-width: 92px;
    height: 36px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border-radius: var(--radio-pq);
    border: 1px solid transparent;
    font: 500 12.5px var(--fuente);
    cursor: pointer;
    transition: border-color 0.12s, filter 0.12s;
  }
  .casilla.permitir {
    background: var(--verde-tinte);
    color: var(--verde);
  }
  .casilla.bloquear {
    background: var(--rojo-tinte);
    color: var(--rojo);
  }
  .casilla:hover {
    filter: brightness(1.2);
  }
  .casilla.elegida,
  .casilla:focus-visible {
    border-color: var(--azul);
    outline: none;
  }
  .num {
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 9px;
    background: var(--superficie-2);
    color: var(--texto);
    font-size: 11px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .marca {
    position: absolute;
    top: 1px;
    right: 6px;
    color: var(--azul);
  }
  .nada {
    color: var(--texto-3);
  }
</style>
