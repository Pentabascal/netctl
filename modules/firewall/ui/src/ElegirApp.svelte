<script>
  // Apps de nDPI con buscador por nombre y categoría, agrupadas por categoría.
  import { Icono } from '@netctl/kit';

  /** apps: GET /apps. valor: [{ id, nombre }]. */
  let { apps, valor = $bindable([]) } = $props();
  let texto = $state('');

  const sinAcentos = (s) => (s ?? '').normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const lista = $derived(apps?.apps ?? []);
  const vistas = $derived.by(() => {
    const q = sinAcentos(texto.trim());
    const l = q ? lista.filter((a) => sinAcentos(`${a.nombre} ${a.categoria}`).includes(q)) : lista;
    const g = new Map();
    for (const a of l) {
      const k = a.categoria || 'Otras';
      if (!g.has(k)) g.set(k, []);
      g.get(k).push(a);
    }
    return [...g.entries()].sort(([a], [b]) => a.localeCompare(b, 'es'));
  });
  const elegida = (id) => valor.some((a) => a.id === id);
  const alternar = (a) => (valor = elegida(a.id) ? valor.filter((x) => x.id !== a.id) : [...valor, { id: a.id, nombre: a.nombre }]);
</script>

{#if valor.length}
  <div class="elegidos">
    {#each valor as a (a.id)}
      <span class="chip">{a.nombre}<button type="button" aria-label="Quitar {a.nombre}" onclick={() => alternar(a)}><Icono nombre="cerrar" tam={11} /></button></span>
    {/each}
  </div>
{/if}
<input class="buscar" type="search" bind:value={texto} placeholder="Busca una app o categoría (vídeo, juegos…)" aria-label="Buscar app" />
<div class="lista" aria-label="Apps">
  {#each vistas as [cat, l] (cat)}
    <span class="cat">{cat}</span>
    {#each l as a (a.id)}
      <label class="app"><input type="checkbox" checked={elegida(a.id)} onchange={() => alternar(a)} /> {a.nombre}</label>
    {/each}
  {:else}
    <span class="nada">{lista.length ? 'Ninguna app con ese nombre.' : 'Cargando las apps…'}</span>
  {/each}
</div>
{#if apps?.version}<span class="pie">nDPI {apps.version}. Bloquea a partir de los primeros paquetes de cada conexión.</span>{/if}

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
    max-height: 240px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .cat {
    margin-top: 6px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--texto-2);
    text-transform: uppercase;
  }
  .app {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 4px;
    border-radius: var(--radio-pq);
    cursor: pointer;
  }
  .app:hover {
    background: var(--hover);
  }
  .app input {
    accent-color: var(--azul);
  }
  .nada,
  .pie {
    color: var(--texto-2);
    font-size: 12px;
  }
</style>
