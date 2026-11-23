<script>
  import { onDestroy, untrack } from 'svelte';
  import Boton from './Boton.svelte';
  import Icono from './Icono.svelte';
  /**
   * Barra fija abajo tras aplicar un cambio que podría dejarte sin acceso. Si no se confirma
   * antes de que acabe la cuenta atrás, el servidor lo deshace solo.
   * `segundos` null = cambio pendiente de antes, sin cuenta atrás conocida.
   * @type {{ segundos?: number|null, mensaje?: string, onconfirmar: () => Promise<void>|void,
   *   onexpirar?: () => void }}
   */
  let {
    segundos = 60,
    mensaje = 'Cambio aplicado. Si sigues viendo el panel, confírmalo; si no, se deshará solo.',
    onconfirmar,
    onexpirar,
  } = $props();

  // La cuenta atrás empieza con el valor inicial; cambiar `segundos` después no la reinicia.
  const total = untrack(() => segundos);
  let quedan = $state(total);
  let confirmando = $state(false);
  let error = $state('');

  const intervalo =
    total != null
      ? setInterval(() => {
          quedan = Math.max(0, quedan - 1);
          if (quedan === 0) {
            clearInterval(intervalo);
            onexpirar?.();
          }
        }, 1000)
      : null;
  onDestroy(() => intervalo && clearInterval(intervalo));

  async function confirmar() {
    confirmando = true;
    error = '';
    try {
      await onconfirmar();
      if (intervalo) clearInterval(intervalo);
    } catch (e) {
      error = e?.mensaje || e?.message || 'No se pudo confirmar';
    } finally {
      confirmando = false;
    }
  }
</script>

<div class="nc-confirmacion" role="alertdialog" aria-live="assertive">
  <div class="progreso" style:width={total ? `${(quedan / total) * 100}%` : '100%'}></div>
  <div class="contenido">
    <div class="reloj" class:urgente={quedan != null && quedan <= 15}>
      {#if total != null}
        <span class="num">{quedan}</span><span class="s">s</span>
      {:else}
        <Icono nombre="reloj" tam={20} />
      {/if}
    </div>
    <div class="texto">
      <strong>Confirma el cambio</strong>
      <span>{error || mensaje}</span>
    </div>
    <Boton variante="primario" icono="ok" cargando={confirmando} onclick={confirmar}>Confirmar cambios</Boton>
  </div>
</div>

<style>
  .nc-confirmacion {
    position: fixed;
    left: 50%;
    bottom: 20px;
    transform: translateX(-50%);
    width: min(720px, calc(100vw - 24px));
    z-index: 60;
    background: var(--superficie-2);
    border: 1px solid var(--azul);
    border-radius: 10px;
    box-shadow: var(--sombra);
    overflow: hidden;
    animation: subir 0.2s ease-out;
  }
  .progreso {
    height: 3px;
    background: var(--azul);
    transition: width 1s linear;
  }
  .contenido {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
  }
  .reloj {
    width: 46px;
    height: 46px;
    flex: none;
    border-radius: 50%;
    display: grid;
    grid-auto-flow: column;
    place-content: center;
    align-items: baseline;
    background: var(--azul-tinte);
    color: var(--azul);
  }
  .reloj.urgente {
    background: var(--ambar-tinte);
    color: var(--ambar);
  }
  .num {
    font-size: 18px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .s {
    font-size: 11px;
    margin-left: 1px;
  }
  .texto {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .texto span {
    color: var(--texto-2);
    font-size: 13px;
  }
  @keyframes subir {
    from {
      transform: translate(-50%, 20px);
      opacity: 0;
    }
  }
  @media (max-width: 560px) {
    .nc-confirmacion {
      bottom: 76px;
    }
    .contenido {
      flex-wrap: wrap;
    }
    .contenido :global(.nc-boton) {
      width: 100%;
    }
  }
</style>
