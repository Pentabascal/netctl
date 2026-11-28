<script>
  import { onMount } from 'svelte';
  import Icono from './Icono.svelte';
  import Ayuda from './Ayuda.svelte';
  import { apiNucleo } from './api.js';
  import { leerEntrada, textoDireccion, buscarEquipos, agruparEquipos } from './validar.js';

  /**
   * Selector de equipos común (§7.6): lista con buscador como en UniFi o IP escrita a mano en el
   * mismo campo. Con `redes`, también redes CIDR, rangos, redes enteras y grupos.
   * `valor`: una dirección (o lista con `varios`) con la forma del modelo de los módulos:
   *   { tipo: 'equipo', mac, ip, nombre } · { tipo: 'ip', red } · { tipo: 'rango', desde, hasta } · { tipo: 'grupo', id }
   * `equipos`: lista ya cargada (si no, se pide `/api/equipos` al núcleo).
   * `redesLista`: [{ nombre, cidr }] para «toda la red». `grupos`: [{ id, nombre }].
   * `propias`: IPs de esta máquina (no se pueden elegir). `conocidas`: CIDR de sus redes.
   * @type {{ etiqueta?: string, valor?: any, varios?: boolean, redes?: boolean, equipos?: any[],
   *   redesLista?: {nombre: string, cidr: string}[], grupos?: {id: number, nombre: string}[],
   *   propias?: string[], conocidas?: string[], ayuda?: string, mas?: any, placeholder?: string,
   *   error?: string, soloIpv4?: boolean, onelegir?: (v: any) => void }}
   */
  let {
    etiqueta = '',
    valor = $bindable(null),
    varios = false,
    redes = false,
    equipos = undefined,
    redesLista = [],
    grupos = [],
    propias = [],
    conocidas = [],
    ayuda = '',
    mas,
    placeholder = 'Busca por nombre, IP o MAC, o escribe una IP',
    error = '',
    soloIpv4 = false,
    onelegir,
  } = $props();

  const id = 'sel-' + Math.random().toString(36).slice(2, 9);
  let cargados = $state([]);
  let errorCarga = $state('');
  let texto = $state('');
  let abierto = $state(false);
  let activo = $state(0);
  let caja = $state();
  let entrada = $state();

  onMount(async () => {
    if (equipos) return;
    try {
      cargados = await apiNucleo.get('/api/equipos');
    } catch (e) {
      errorCarga = 'No se pudo cargar la lista de equipos; puedes escribir la IP a mano.';
    }
  });

  const lista = $derived(
    (equipos ?? cargados).filter((e) => !soloIpv4 || !String(e.ip).includes(':')),
  );
  const elegidos = $derived(varios ? (valor ?? []) : valor ? [valor] : []);
  const nombresGrupos = $derived(Object.fromEntries(grupos.map((g) => [g.id, g.nombre])));

  /** Dirección de un equipo de la lista: vinculada por MAC si la tiene. */
  function dirDe(e) {
    if (e.mac) return { tipo: 'equipo', mac: e.mac, ip: e.ip, nombre: e.nombre || e.ip };
    return { tipo: 'ip', red: `${e.ip}/${String(e.ip).includes(':') ? 128 : 32}` };
  }

  const igual = (a, b) => JSON.stringify(a) === JSON.stringify(b);

  const manual = $derived.by(() => {
    const t = texto.trim();
    if (!t) return null;
    const r = leerEntrada(t, { redes, propias, conocidas });
    if (!r.ok) return /[\d:]/.test(t[0] ?? '') ? r : null;
    const ip = r.dir.red ?? r.dir.desde ?? '';
    if (soloIpv4 && ip.includes(':')) return { ok: false, error: 'Aquí solo vale IPv4.' };
    return r;
  });

  /** Opciones de la lista desplegable, en orden, para moverse con el teclado. */
  const opciones = $derived.by(() => {
    const v = [];
    if (manual?.ok && !lista.some((e) => e.ip === texto.trim())) {
      v.push({ clase: 'manual', dir: manual.dir, aviso: manual.aviso });
    }
    for (const g of agruparEquipos(buscarEquipos(lista, texto))) {
      v.push({ clase: 'titulo', red: g.red });
      for (const e of g.equipos) v.push({ clase: 'equipo', e, dir: dirDe(e) });
    }
    if (redes) {
      const t = texto.trim().toLowerCase();
      for (const r of redesLista.filter((r) => !t || r.nombre.toLowerCase().includes(t) || r.cidr.includes(t))) {
        v.push({ clase: 'red', r, dir: { tipo: 'ip', red: r.cidr } });
      }
      for (const g of grupos.filter((g) => !t || g.nombre.toLowerCase().includes(t))) {
        v.push({ clase: 'grupo', g, dir: { tipo: 'grupo', id: g.id } });
      }
    }
    return v;
  });
  const elegibles = $derived(opciones.map((o, i) => [o, i]).filter(([o]) => o.clase !== 'titulo'));

  /** Si se escribió una IP a mano que es de un equipo conocido, se ofrece vincularla. */
  const vinculables = $derived(
    elegidos
      .map((d, i) => {
        if (d.tipo !== 'ip') return null;
        const ip = d.red.replace(/\/(32|128)$/, '');
        const e = lista.find((x) => x.ip === ip && x.mac);
        return e ? { i, e } : null;
      })
      .filter(Boolean),
  );

  function elegir(dir) {
    if (varios) {
      if (!elegidos.some((x) => igual(x, dir))) valor = [...elegidos, dir];
    } else {
      valor = dir;
      abierto = false;
    }
    texto = '';
    activo = 0;
    onelegir?.(valor);
    entrada?.focus();
  }

  function quitar(i) {
    if (varios) valor = elegidos.filter((_, j) => j !== i);
    else valor = null;
    onelegir?.(valor);
  }

  function vincular(i, e) {
    const nuevo = dirDe(e);
    if (varios) valor = elegidos.map((d, j) => (j === i ? nuevo : d));
    else valor = nuevo;
    onelegir?.(valor);
  }

  function tecla(ev) {
    if (ev.key === 'ArrowDown' || ev.key === 'ArrowUp') {
      ev.preventDefault();
      abierto = true;
      const n = elegibles.length;
      if (!n) return;
      const pos = elegibles.findIndex(([, i]) => i === activo);
      const sig = ev.key === 'ArrowDown' ? (pos + 1) % n : (pos - 1 + n) % n;
      activo = elegibles[sig][1];
    } else if (ev.key === 'Enter') {
      ev.preventDefault();
      const o = opciones[activo]?.clase !== 'titulo' ? opciones[activo] : elegibles[0]?.[0];
      if (o) elegir(o.dir);
    } else if (ev.key === 'Escape') {
      abierto = false;
    } else if (ev.key === 'Backspace' && !texto && elegidos.length) {
      quitar(elegidos.length - 1);
    }
  }

  function fuera(ev) {
    if (abierto && caja && !caja.contains(ev.target)) abierto = false;
  }

  $effect(() => {
    // Al cambiar el texto, la primera opción elegible queda marcada.
    texto;
    activo = elegibles[0]?.[1] ?? 0;
  });

  function iconoDe(d) {
    if (d.tipo === 'equipo') return 'red';
    if (d.tipo === 'grupo') return 'clientes';
    if (d.tipo === 'rango' || (d.tipo === 'ip' && !/\/(32|128)$/.test(d.red))) return 'casa_red';
    return 'editar';
  }
</script>

<svelte:window onclick={fuera} />

<div class="nc-selector" class:con-error={!!(error || (manual && !manual.ok))} bind:this={caja}>
  {#if etiqueta}
    <label for={id}
      >{etiqueta}{#if mas}<Ayuda texto={mas.texto} ejemplo={mas.ejemplo} recomendado={mas.recomendado} />{/if}</label
    >
  {/if}
  <div
    class="control"
    onclick={() => {
      abierto = true;
      entrada?.focus();
    }}
    role="presentation"
  >
    {#each elegidos as d, i (JSON.stringify(d))}
      <span class="ficha" class:mano={d.tipo === 'ip' && /\/(32|128)$/.test(d.red)}>
        <Icono nombre={iconoDe(d)} tam={13} />
        <span class="ficha-texto">{textoDireccion(d, nombresGrupos)}</span>
        <button
          type="button"
          aria-label="Quitar {textoDireccion(d, nombresGrupos)}"
          onclick={(ev) => {
            ev.stopPropagation();
            quitar(i);
          }}><Icono nombre="cerrar" tam={12} /></button
        >
      </span>
    {/each}
    {#if varios || !elegidos.length}
      <input
        {id}
        bind:this={entrada}
        bind:value={texto}
        placeholder={elegidos.length ? 'Añadir otro…' : placeholder}
        autocomplete="off"
        spellcheck="false"
        role="combobox"
        aria-expanded={abierto}
        aria-controls="{id}-lista"
        onfocus={() => (abierto = true)}
        onkeydown={tecla}
      />
    {/if}
  </div>

  {#if abierto && (opciones.length || texto)}
    <ul class="lista" id="{id}-lista" role="listbox">
      {#each opciones as o, i (i)}
        {#if o.clase === 'titulo'}
          <li class="titulo" role="presentation">{o.red === 'VPN' ? 'VPN' : `Red ${o.red}`}</li>
        {:else}
          <li
            role="option"
            aria-selected={i === activo}
            class:activa={i === activo}
            onmouseenter={() => (activo = i)}
            onmousedown={(ev) => {
              ev.preventDefault();
              elegir(o.dir);
            }}
          >
            {#if o.clase === 'manual'}
              <span class="ico"><Icono nombre="editar" tam={15} /></span>
              <span class="txt"><b>Usar {textoDireccion(o.dir)}</b> <small>(escrita a mano)</small>
                {#if o.aviso}<small class="aviso">{o.aviso}</small>{/if}</span
              >
            {:else if o.clase === 'equipo'}
              <span class="ico"><Icono nombre={o.e.origen === 'vpn' ? 'movil' : 'red'} tam={15} /></span>
              <span class="txt">
                <b>{o.e.nombre || 'Equipo sin nombre'}</b>
                <small>{o.e.ip}{#if o.e.mac}{' · '}<span class="mac">{o.e.mac}</span>{/if}{#if o.e.fabricante}{' · '}{o.e.fabricante}{/if}</small>
              </span>
              <span
                class="estado"
                class:on={o.e.conectado}
                class:off={o.e.desactivado}
                title={o.e.desactivado ? 'Desactivado' : o.e.conectado ? 'Conectado' : 'Desconectado'}
                >{o.e.desactivado ? '⊘' : o.e.conectado ? '●' : '○'}</span
              >
            {:else if o.clase === 'red'}
              <span class="ico"><Icono nombre="casa_red" tam={15} /></span>
              <span class="txt"><b>Toda la red {o.r.nombre}</b> <small>{o.r.cidr}</small></span>
            {:else}
              <span class="ico"><Icono nombre="clientes" tam={15} /></span>
              <span class="txt"><b>Grupo «{o.g.nombre}»</b></span>
            {/if}
          </li>
        {/if}
      {/each}
      {#if !elegibles.length}
        <li class="vacio" role="presentation">
          {manual && !manual.ok ? manual.error : 'Ningún equipo coincide; puedes escribir una IP.'}
        </li>
      {/if}
    </ul>
  {/if}

  {#each vinculables as v (v.i)}
    <p class="vincular">
      {textoDireccion(elegidos[v.i])} es «{v.e.nombre || v.e.mac}».
      <button type="button" onclick={() => vincular(v.i, v.e)}>Vincular al equipo</button>
      <span>(la regla le seguirá aunque cambie de IP)</span>
    </p>
  {/each}

  {#if error}
    <p class="error">{error}</p>
  {:else if manual && !manual.ok && !abierto}
    <p class="error">{manual.error}</p>
  {:else if errorCarga}
    <p class="ayuda">{errorCarga}</p>
  {:else if ayuda}
    <p class="ayuda">{ayuda}</p>
  {/if}
</div>

<style>
  .nc-selector {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  label {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--texto-2);
  }
  .control {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-height: var(--alto-campo);
    padding: 4px 8px;
    background: var(--campo);
    border: 1px solid var(--borde-fuerte);
    border-radius: var(--radio-pq);
    cursor: text;
  }
  .control:focus-within {
    border-color: var(--azul);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--azul) 22%, transparent);
  }
  .con-error .control {
    border-color: var(--rojo);
  }
  input {
    flex: 1;
    min-width: 140px;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--texto);
    font: inherit;
    height: 24px;
  }
  input::placeholder {
    color: var(--texto-3);
  }
  .ficha {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 24px;
    padding: 0 4px 0 8px;
    background: var(--azul-tinte);
    color: var(--texto);
    border-radius: 12px;
    font-size: 12.5px;
    max-width: 100%;
  }
  .ficha.mano {
    background: var(--superficie-2);
    border: 1px dashed var(--borde-fuerte);
  }
  .ficha-texto {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ficha button {
    display: inline-flex;
    border: 0;
    background: transparent;
    color: var(--texto-2);
    cursor: pointer;
    padding: 3px;
    border-radius: 50%;
  }
  .ficha button:hover {
    color: var(--texto);
    background: var(--hover);
  }
  .lista {
    position: absolute;
    z-index: 50;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    max-height: 300px;
    overflow: auto;
    margin: 0;
    padding: 4px;
    list-style: none;
    background: var(--superficie-2);
    border: 1px solid var(--borde-fuerte);
    border-radius: var(--radio);
    box-shadow: var(--sombra);
  }
  label + .control + .lista {
    top: calc(100% + 4px);
  }
  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 8px;
    border-radius: var(--radio-pq);
    cursor: pointer;
  }
  li.activa {
    background: var(--hover);
  }
  li.titulo {
    cursor: default;
    padding: 8px 8px 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--texto-3);
  }
  li.vacio {
    cursor: default;
    color: var(--texto-3);
    font-size: 12.5px;
  }
  .ico {
    display: inline-flex;
    color: var(--texto-2);
  }
  .txt {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .txt b {
    font-weight: 500;
  }
  small {
    color: var(--texto-3);
    font-size: 11.5px;
  }
  .mac {
    font-family: var(--mono);
    font-size: 10.5px;
  }
  small.aviso {
    color: var(--ambar);
  }
  .estado {
    color: var(--texto-3);
    font-size: 12px;
  }
  .estado.on {
    color: var(--verde);
  }
  .vincular {
    font-size: 12px;
    color: var(--texto-2);
  }
  .vincular button {
    border: 0;
    background: none;
    color: var(--azul);
    cursor: pointer;
    font: inherit;
    padding: 0 4px;
  }
  .vincular span {
    color: var(--texto-3);
  }
  .ayuda,
  .error {
    font-size: 12px;
    color: var(--texto-3);
  }
  .error {
    color: var(--rojo);
  }
</style>
