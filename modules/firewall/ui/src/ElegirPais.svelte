<script>
  // Países con buscador: bandera, nombre en español y casilla. Abajo la atribución de DB-IP
  // (CC BY 4.0) y el estado de la base.
  import { Icono } from '@netctl/kit';
  import { bandera } from './politica.js';

  /** geo: GET /geo. valor: códigos elegidos. */
  let { geo, valor = $bindable([]) } = $props();
  let texto = $state('');

  const nombres = (() => {
    try {
      return new Intl.DisplayNames(['es'], { type: 'region' });
    } catch {
      return null;
    }
  })();
  const nombre = (cc) => nombres?.of(cc) ?? cc;
  const sinAcentos = (s) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();

  const todos = $derived((geo?.paises ?? []).map((p) => ({ cc: p.codigo, nombre: nombre(p.codigo) })).sort((a, b) => a.nombre.localeCompare(b.nombre, 'es')));
  const vistos = $derived(texto.trim() ? todos.filter((p) => sinAcentos(`${p.nombre} ${p.cc}`).includes(sinAcentos(texto.trim()))) : todos);

  const alternar = (cc) => (valor = valor.includes(cc) ? valor.filter((x) => x !== cc) : [...valor, cc]);
</script>

{#if valor.length}
  <div class="elegidos">
    {#each valor as cc (cc)}
      <span class="chip">{bandera(cc)} {nombre(cc)}<button type="button" aria-label="Quitar {nombre(cc)}" onclick={() => alternar(cc)}><Icono nombre="cerrar" tam={11} /></button></span>
    {/each}
  </div>
{/if}
<input class="buscar" type="search" bind:value={texto} placeholder="Busca un país" aria-label="Buscar país" />
<div class="lista" role="listbox" aria-multiselectable="true" aria-label="Países">
  {#each vistos as p (p.cc)}
    <label class="pais"><input type="checkbox" checked={valor.includes(p.cc)} onchange={() => alternar(p.cc)} /> <span class="bandera">{bandera(p.cc)}</span> {p.nombre}</label>
  {:else}
    <span class="nada">Ningún país con ese nombre.</span>
  {/each}
</div>
<span class="pie">
  {#if geo?.estado === 'lista'}Base de {geo.fecha}.{:else if geo?.estado === 'descargando'}Descargando la base de países…{:else}La base se descarga al guardar la primera regla con países.{/if}
  <a href={geo?.enlace ?? 'https://db-ip.com'} target="_blank" rel="noopener">IP Geolocation by DB-IP</a>
</span>

<style>
  .elegidos {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 4px 2px 8px;
    background: var(--azul-tinte);
    border-radius: 12px;
    font-size: 12.5px;
  }
  .chip button {
    background: none;
    border: 0;
    color: var(--texto-2);
    cursor: pointer;
    display: grid;
    place-items: center;
  }
  .buscar {
    height: 32px;
    padding: 0 10px;
    background: var(--campo);
    border: 0;
    border-radius: var(--radio-pq);
    color: var(--texto);
    font: inherit;
  }
  .lista {
    max-height: 220px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px 0;
  }
  .pais {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 4px;
    border-radius: var(--radio-pq);
    cursor: pointer;
  }
  .pais:hover {
    background: var(--hover);
  }
  .pais input {
    accent-color: var(--azul);
  }
  .bandera {
    font-size: 15px;
  }
  .nada,
  .pie {
    color: var(--texto-2);
    font-size: 12px;
  }
  .pie a {
    color: var(--azul);
    margin-left: 4px;
  }
</style>
