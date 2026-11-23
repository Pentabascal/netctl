<script>
  import Icono from './Icono.svelte';
  /**
   * tipo: info | ok | aviso | error
   * @type {{ tipo?: string, titulo?: string, cerrable?: boolean, oncerrar?: () => void,
   *   children?: any, acciones?: any }}
   */
  let { tipo = 'info', titulo = '', cerrable = false, oncerrar, children, acciones } = $props();
  const iconos = { info: 'info', ok: 'ok_circulo', aviso: 'aviso', error: 'error' };
</script>

<div class="nc-aviso {tipo}" role={tipo === 'error' ? 'alert' : 'status'}>
  <span class="icono"><Icono nombre={iconos[tipo] ?? 'info'} tam={18} /></span>
  <div class="cuerpo">
    {#if titulo}<strong>{titulo}</strong>{/if}
    {#if children}<div class="texto">{@render children()}</div>{/if}
  </div>
  {#if acciones}<div class="acciones">{@render acciones()}</div>{/if}
  {#if cerrable}
    <button class="cerrar" type="button" aria-label="Cerrar" onclick={oncerrar}><Icono nombre="cerrar" tam={16} /></button>
  {/if}
</div>

<style>
  .nc-aviso {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px 14px;
    border-radius: var(--radio);
    border: 1px solid var(--borde);
    border-left-width: 3px;
    background: var(--superficie);
  }
  .icono {
    margin-top: 1px;
    display: flex;
  }
  .cuerpo {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  strong {
    font-weight: 600;
  }
  .texto {
    color: var(--texto-2);
    overflow-wrap: anywhere;
  }
  .acciones {
    display: flex;
    gap: 8px;
    align-self: center;
  }
  .info {
    border-left-color: var(--azul);
  }
  .info .icono {
    color: var(--azul);
  }
  .ok {
    border-left-color: var(--verde);
  }
  .ok .icono {
    color: var(--verde);
  }
  .aviso {
    border-left-color: var(--ambar);
  }
  .aviso .icono {
    color: var(--ambar);
  }
  .error {
    border-left-color: var(--rojo);
    background: color-mix(in srgb, var(--rojo-tinte) 40%, var(--superficie));
  }
  .error .icono {
    color: var(--rojo);
  }
  .cerrar {
    background: none;
    border: 0;
    color: var(--texto-2);
    cursor: pointer;
    padding: 2px;
    border-radius: 4px;
    display: flex;
  }
  .cerrar:hover {
    color: var(--texto);
    background: var(--hover);
  }
  @media (max-width: 560px) {
    .nc-aviso {
      flex-wrap: wrap;
    }
    .acciones {
      width: 100%;
      padding-left: 30px;
    }
  }
</style>
