<script>
  // Ajustes como en UniFi: segundo menú a la izquierda (buscador, secciones del núcleo y los
  // grupos que aportan los módulos con [[ajustes]], como pestañas con icono) y la página a la derecha.
  import { Icono } from '@netctl/kit';
  import { sesion } from '../sesion.svelte.js';
  import { ruta, analizar } from '../ruta.svelte.js';
  import AjustesNucleo from './AjustesNucleo.svelte';
  import ModuloUI from '../ModuloUI.svelte';

  const NUCLEO = [
    { id: 'password', titulo: 'Contraseña', icono: 'llave' },
    { id: 'totp', titulo: 'Verificación en dos pasos', icono: 'movil' },
    { id: 'acceso', titulo: 'Acceso al panel', icono: 'candado' },
  ];

  const vista = $derived(analizar(ruta.actual));
  let buscar = $state('');

  const sinAcentos = (s) => (s ?? '').normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
  const casa = (t) => !buscar.trim() || sinAcentos(t).includes(sinAcentos(buscar.trim()));

  // [{ grupo, entradas: [{ id, ruta, titulo, icono, href }] }] en el orden en que llegan.
  const grupos = $derived.by(() => {
    const g = [];
    for (const m of sesion.modulos) {
      for (const e of m.ajustes ?? []) {
        let x = g.find((y) => y.grupo === e.grupo);
        if (!x) g.push((x = { grupo: e.grupo, entradas: [] }));
        x.entradas.push({ ...e, id: m.id, icono: e.icono || m.icono, href: `/ajustes/m/${m.id}/${e.ruta.replace(/^\/+/, '')}` });
      }
    }
    return g;
  });

  const seccion = $derived(vista.seccion === 'm' ? 'm' : NUCLEO.some((s) => s.id === vista.seccion) ? vista.seccion : 'password');
  const activa = (e) => vista.seccion === 'm' && vista.id === e.id && (vista.sub || '').split('/')[0] === e.ruta.replace(/^\/+/, '');
</script>

<div class="ajustes">
  <nav class="menu" aria-label="Ajustes">
    <label class="buscar">
      <Icono nombre="buscar" tam={14} />
      <input type="search" placeholder="Buscar ajustes" bind:value={buscar} aria-label="Buscar ajustes" />
    </label>

    {#each NUCLEO.filter((s) => casa(s.titulo)) as s (s.id)}
      <a class="item" class:activa={seccion === s.id} href="/ajustes/{s.id}" aria-current={seccion === s.id ? 'page' : undefined}>
        <Icono nombre={s.icono} tam={16} />{s.titulo}
      </a>
    {/each}

    {#each grupos as g (g.grupo)}
      {@const visibles = g.entradas.filter((e) => casa(e.titulo) || casa(g.grupo))}
      {#if visibles.length}
        <div class="grupo">
          <span class="rotulo">{g.grupo}</span>
          <div class="iconos">
            {#each visibles as e (e.href)}
              <a class="pestana" class:activa={activa(e)} href={e.href} title={e.titulo} aria-label={e.titulo} aria-current={activa(e) ? 'page' : undefined}>
                <Icono nombre={e.icono} tam={17} />
              </a>
            {/each}
          </div>
          {#if visibles.some(activa)}<span class="actual">{visibles.find(activa).titulo}</span>{/if}
        </div>
      {/if}
    {/each}
  </nav>

  <div class="pagina">
    {#if seccion === 'm'}
      {#key vista.id}
        <ModuloUI id={vista.id} sub={vista.sub} base="/ajustes/m/{vista.id}" />
      {/key}
    {:else}
      <AjustesNucleo {seccion} />
    {/if}
  </div>
</div>

<style>
  .ajustes {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    min-height: 100%;
  }
  .menu {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 14px 10px;
    border-right: 1px solid var(--borde-suave);
  }
  .buscar {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 10px;
    margin-bottom: 10px;
    background: var(--campo);
    border-radius: var(--radio-pq);
    color: var(--texto-3);
  }
  .buscar input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    color: var(--texto);
    font: inherit;
    outline: none;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 34px;
    padding: 0 10px;
    border-radius: var(--radio-pq);
    color: var(--texto);
  }
  .item:hover,
  .pestana:hover {
    background: var(--hover);
    text-decoration: none;
  }
  .item.activa {
    background: var(--superficie);
    color: var(--azul);
  }
  .grupo {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 10px;
    padding: 10px 0 10px;
    border-top: 1px solid var(--borde-suave);
  }
  .rotulo {
    padding: 0 10px;
    font-size: 11.5px;
    color: var(--texto-2);
  }
  .iconos {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 0 6px;
  }
  .pestana {
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    border-radius: var(--radio-pq);
    color: var(--texto-2);
  }
  .pestana.activa {
    background: var(--superficie);
    color: var(--azul);
  }
  .actual {
    padding: 0 10px;
    font-size: 12px;
    color: var(--azul);
  }
  .pagina {
    min-width: 0;
  }
  @media (max-width: 760px) {
    .ajustes {
      grid-template-columns: 1fr;
    }
    .menu {
      border-right: 0;
      border-bottom: 1px solid var(--borde-suave);
    }
  }
</style>
