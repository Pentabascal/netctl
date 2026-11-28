<script>
  // Tabla de zonas (UniFi): nombre con candado si es de serie y sus tarjetas. Debajo, «Crear zona».
  import { Icono, Chip, Ayuda, Enlace, Aviso } from '@netctl/kit';
  import { NOMBRE_ZONA_TIPO } from './estado.svelte.js';

  let { zonas, tarjetas, sinZona = [], oneditar, oncrear, onajustes } = $props();
</script>

<div class="tabla">
  <div class="fila cab">
    <span>Nombre de la zona</span>
    <span>Tarjetas de red</span>
  </div>
  {#each zonas as z (z.id)}
    <button type="button" class="fila" onclick={() => oneditar(z)} title="Cambiar las tarjetas de «{z.nombre}»">
      <span class="nombre">
        {z.nombre}
        {#if z.tipo !== 'personalizada'}<span class="candado" title="Zona de serie"><Icono nombre="candado" tam={13} /></span>{/if}
        <Ayuda texto={NOMBRE_ZONA_TIPO[z.tipo]} titulo="Qué es «{z.nombre}»" />
      </span>
      <span class="chips">
        {#each tarjetas[z.id] ?? [] as t (t)}<span class="chip">{t}</span>{:else}<span class="nada">—</span>{/each}
      </span>
    </button>
  {/each}
</div>
{#if sinZona.length}
  <Aviso tipo="aviso">
    {sinZona.length === 1 ? 'La tarjeta' : 'Las tarjetas'} <b>{sinZona.join(', ')}</b> no {sinZona.length === 1 ? 'tiene' : 'tienen'} zona:
    todo lo que entre por {sinZona.length === 1 ? 'ella' : 'ellas'} se bloquea. Pulsa una zona para añadírsela.
  </Aviso>
{/if}
<div class="acciones">
  <Enlace icono="mas" onclick={oncrear}>Crear zona</Enlace>
  <Enlace icono="ajustes" onclick={onajustes}>Ajustes del firewall</Enlace>
</div>

<style>
  .tabla {
    display: flex;
    flex-direction: column;
  }
  .fila {
    display: grid;
    grid-template-columns: 220px 1fr;
    align-items: center;
    min-height: var(--alto-fila);
    padding: 0 8px;
    border: 0;
    border-bottom: 1px solid var(--borde-suave);
    background: none;
    color: var(--texto);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .fila:not(.cab):hover {
    background: var(--hover);
  }
  .cab {
    font-weight: 600;
    cursor: default;
  }
  .nombre {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .candado {
    color: var(--texto-2);
    display: inline-flex;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chip {
    padding: 1px 6px;
    border-radius: 3px;
    background: var(--superficie-2);
    font-size: 12px;
  }
  .nada {
    color: var(--texto-3);
  }
  .acciones {
    display: flex;
    gap: 18px;
    padding: 4px 8px;
  }
</style>
