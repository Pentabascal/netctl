<script>
  // Lista de dominios como chips: se escribe uno y Intro (o coma) lo añade si es válido.
  import { Icono } from '@netctl/kit';
  import { esDominio } from './politica.js';

  let { valor = $bindable([]) } = $props();
  let texto = $state('');
  let error = $state('');

  function anadir() {
    const partes = texto.split(/[\s,]+/).map((x) => x.trim().toLowerCase()).filter(Boolean);
    const malos = partes.filter((x) => !esDominio(x));
    valor = [...valor, ...partes.filter((x) => esDominio(x) && !valor.includes(x))];
    texto = malos.join(' ');
    error = malos.length ? `«${malos[0]}» no es un dominio válido.` : '';
  }
  function tecla(e) {
    if (e.key === 'Enter' || e.key === ',') {
      e.preventDefault();
      anadir();
    } else if (e.key === 'Backspace' && !texto && valor.length) valor = valor.slice(0, -1);
  }
</script>

<div class="caja" class:mal={!!error}>
  {#each valor as d (d)}
    <span class="chip">{d}<button type="button" aria-label="Quitar {d}" onclick={() => (valor = valor.filter((x) => x !== d))}><Icono nombre="cerrar" tam={11} /></button></span>
  {/each}
  <input bind:value={texto} onkeydown={tecla} onblur={() => texto.trim() && anadir()} placeholder={valor.length ? '' : 'tiktok.com, youtube.com…'} aria-label="Dominios" />
</div>
{#if error}<span class="error">{error}</span>{:else}<span class="pista">Escribe un dominio y pulsa Intro. Incluye los subdominios que uses (www.…).</span>{/if}

<style>
  .caja {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 6px 8px;
    min-height: 36px;
    background: var(--campo);
    border-radius: var(--radio-pq);
    border: 1px solid transparent;
  }
  .caja.mal {
    border-color: var(--rojo);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 4px 2px 8px;
    background: var(--azul-tinte);
    border-radius: 12px;
    font-family: var(--mono);
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
  input {
    flex: 1;
    min-width: 140px;
    background: none;
    border: 0;
    color: var(--texto);
    font: inherit;
    outline: none;
  }
  .error {
    color: var(--rojo);
    font-size: 12px;
  }
  .pista {
    color: var(--texto-2);
    font-size: 12px;
  }
</style>
