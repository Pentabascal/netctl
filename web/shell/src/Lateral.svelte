<script>
  import { Icono } from '@netctl/kit';
  import { sesion } from './sesion.svelte.js';

  let { vista } = $props();

  // Entradas: Inicio, después las de cada módulo (de su manifiesto) y abajo Módulos y Ajustes.
  const entradasModulos = $derived(
    // Un módulo que solo aporta páginas a Ajustes (p. ej. el Firewall) no sale aquí.
    sesion.modulos.filter((m) => m.menu?.length || !m.ajustes?.length).map((m) => ({
      id: m.id,
      nombre: m.nombre,
      entradas: (m.menu?.length ? m.menu : [{ titulo: m.nombre, ruta: '', icono: m.icono }]).map((e) => ({
        href: `/modulo/${m.id}${e.ruta ? '/' + e.ruta.replace(/^\/+/, '') : ''}`,
        titulo: m.menu?.length > 1 ? `${m.nombre} · ${e.titulo}` : e.titulo,
        icono: e.icono || m.icono,
        activa: vista.pagina === 'modulo' && vista.id === m.id && (vista.sub || '') === (e.ruta || '').replace(/^\/+/, ''),
      })),
    })),
  );
</script>

<nav class="lateral" aria-label="Menú principal">
  <a class="logo" href="/" aria-label="netctl, inicio">
    <svg viewBox="0 0 32 32" width="30" height="30" aria-hidden="true">
      <rect width="32" height="32" rx="8" fill="var(--azul)" />
      <g fill="none" stroke="#fff" stroke-width="2.2" stroke-linecap="round">
        <circle cx="16" cy="16" r="3" />
        <path d="M16 6v5M16 21v5M6 16h5M21 16h5" />
      </g>
    </svg>
  </a>

  <div class="grupo">
    <a class="item" class:activa={vista.pagina === 'inicio'} href="/" aria-current={vista.pagina === 'inicio' ? 'page' : undefined}>
      <Icono nombre="inicio" tam={20} /><span class="pista">Inicio</span>
    </a>
  </div>

  {#each entradasModulos as m (m.id)}
    <div class="grupo modulo">
      {#each m.entradas as e (e.href)}
        <a class="item" class:activa={e.activa} href={e.href} aria-current={e.activa ? 'page' : undefined}>
          <Icono nombre={e.icono} tam={20} /><span class="pista">{e.titulo}</span>
        </a>
      {/each}
    </div>
  {/each}

  <div class="relleno"></div>

  <div class="grupo abajo">
    <a class="item" class:activa={vista.pagina === 'modulos'} href="/modulos">
      <Icono nombre="modulos" tam={20} /><span class="pista">Módulos</span>
    </a>
    <a class="item" class:activa={vista.pagina === 'ajustes'} href="/ajustes">
      <Icono nombre="ajustes" tam={20} /><span class="pista">Ajustes</span>
    </a>
  </div>
</nav>

<style>
  .lateral {
    position: fixed;
    top: 0;
    left: 0;
    bottom: 0;
    width: var(--ancho-lateral);
    z-index: 30;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 12px 0 14px;
    background: var(--fondo-lateral);
    border-right: 1px solid var(--borde-suave);
  }
  .logo {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    margin-bottom: 10px;
    border-radius: 10px;
  }
  .grupo {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 100%;
    align-items: center;
  }
  .modulo {
    padding-top: 6px;
    margin-top: 2px;
    border-top: 1px solid var(--borde-suave);
    width: 36px;
  }
  .relleno {
    flex: 1;
  }
  .item {
    position: relative;
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    border-radius: 10px;
    color: var(--texto-2);
    transition: background 0.12s, color 0.12s;
  }
  .item:hover {
    background: var(--hover);
    color: var(--texto);
    text-decoration: none;
  }
  .item.activa {
    background: var(--azul-tinte);
    color: var(--azul);
  }
  .item.activa::before {
    content: '';
    position: absolute;
    left: -11px;
    top: 10px;
    bottom: 10px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--azul);
  }
  .pista {
    position: absolute;
    left: calc(100% + 12px);
    top: 50%;
    transform: translateY(-50%) translateX(-4px);
    padding: 5px 10px;
    border-radius: 6px;
    background: var(--superficie-2);
    border: 1px solid var(--borde);
    color: var(--texto);
    font-size: 12.5px;
    white-space: nowrap;
    box-shadow: var(--sombra);
    pointer-events: none;
    opacity: 0;
    transition: opacity 0.12s, transform 0.12s;
  }
  .item:hover .pista,
  .item:focus-visible .pista {
    opacity: 1;
    transform: translateY(-50%);
  }

  @media (max-width: 720px) {
    .lateral {
      top: auto;
      right: 0;
      width: auto;
      height: 60px;
      flex-direction: row;
      padding: 0 8px env(safe-area-inset-bottom);
      gap: 2px;
      border-right: 0;
      border-top: 1px solid var(--borde);
      overflow-x: auto;
      justify-content: space-around;
    }
    .logo,
    .relleno,
    .pista {
      display: none;
    }
    .grupo {
      flex-direction: row;
      width: auto;
      gap: 2px;
    }
    .modulo {
      border-top: 0;
      padding-top: 0;
      margin-top: 0;
      width: auto;
    }
    .item.activa::before {
      left: 10px;
      right: 10px;
      top: -9px;
      bottom: auto;
      width: auto;
      height: 3px;
      border-radius: 0 0 3px 3px;
    }
  }
</style>
