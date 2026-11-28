<script>
  import { Icono, Chip } from '@netctl/kit';
  import { sesion, salir } from './sesion.svelte.js';
  let abierto = $state(false);
</script>

<svelte:window onclick={() => (abierto = false)} />

<header class="cabecera">
  <div class="marca">
    <strong>netctl</strong>
    <span class="sep"></span>
    <span class="sub">Panel de red</span>
  </div>
  <div class="nc-separador"></div>
  {#if sesion.yo?.alcance === 'internet'}
    <Chip color="ambar">Accesible desde Internet</Chip>
  {/if}
  <div class="usuario">
    <button
      type="button"
      class="boton-usuario"
      aria-haspopup="menu"
      aria-expanded={abierto}
      onclick={(e) => {
        e.stopPropagation();
        abierto = !abierto;
      }}
    >
      <span class="avatar"><Icono nombre="usuario" tam={16} /></span>
      <span class="nombre">{sesion.yo?.usuario}</span>
      <Icono nombre="chevron_abajo" tam={14} />
    </button>
    {#if abierto}
      <div class="menu" role="menu">
        <div class="info">
          <span>Sesión de <strong>{sesion.yo?.usuario}</strong></span>
          <span class="ver">netctl {sesion.yo?.version}</span>
        </div>
        <a role="menuitem" href="/ajustes"><Icono nombre="ajustes" tam={16} />Ajustes</a>
        <button role="menuitem" type="button" onclick={salir}><Icono nombre="salir" tam={16} />Cerrar sesión</button>
      </div>
    {/if}
  </div>
</header>

<style>
  .cabecera {
    position: sticky;
    top: 0;
    z-index: 20;
    height: var(--alto-cabecera);
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 20px 0 28px;
    background: var(--cabecera);
    backdrop-filter: blur(8px);
    border-bottom: 1px solid var(--borde-suave);
  }
  .marca {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .marca strong {
    font-size: 15px;
    letter-spacing: -0.01em;
  }
  .sep {
    width: 1px;
    height: 16px;
    background: var(--borde);
  }
  .sub {
    color: var(--texto-2);
    font-size: 13px;
  }
  .usuario {
    position: relative;
  }
  .boton-usuario {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 8px 0 4px;
    border-radius: 18px;
    background: none;
    border: 1px solid transparent;
    color: var(--texto-2);
    font: inherit;
    cursor: pointer;
  }
  .boton-usuario:hover,
  .boton-usuario[aria-expanded='true'] {
    background: var(--hover);
    border-color: var(--borde);
    color: var(--texto);
  }
  .avatar {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--azul-tinte);
    color: var(--azul);
  }
  .nombre {
    color: var(--texto);
    font-weight: 500;
  }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    min-width: 210px;
    padding: 6px;
    background: var(--superficie-2);
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    box-shadow: var(--sombra);
    display: flex;
    flex-direction: column;
  }
  .info {
    display: flex;
    flex-direction: column;
    padding: 8px 10px 10px;
    border-bottom: 1px solid var(--borde-suave);
    margin-bottom: 4px;
    font-size: 13px;
  }
  .ver {
    color: var(--texto-3);
    font-size: 12px;
  }
  .menu a,
  .menu button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 6px;
    background: none;
    border: 0;
    color: var(--texto);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .menu a:hover,
  .menu button:hover {
    background: var(--hover);
    text-decoration: none;
  }
  @media (max-width: 720px) {
    .cabecera {
      padding: 0 12px 0 16px;
    }
    .sub,
    .sep,
    .nombre {
      display: none;
    }
  }
</style>
