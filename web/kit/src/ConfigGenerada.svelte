<script>
  import Boton from './Boton.svelte';
  /**
   * Configuración que genera un módulo, en solo lectura y con botón Copiar (§3b).
   * @type {{ texto?: string, frase?: string }}
   */
  let { texto = '', frase = 'Esto es lo que netctl carga en el sistema. No hace falta tocarlo: se genera solo.' } = $props();
  let copiado = $state(false);
  async function copiar() {
    try {
      await navigator.clipboard.writeText(texto);
      copiado = true;
      setTimeout(() => (copiado = false), 1800);
    } catch {
      copiado = false;
    }
  }
</script>

<div class="nc-generada">
  <div class="cab">
    <p>{frase}</p>
    <Boton tam="pequeno" icono={copiado ? 'ok' : 'copiar'} onclick={copiar}>{copiado ? 'Copiado' : 'Copiar'}</Boton>
  </div>
  <pre>{texto || 'Todavía no hay nada generado.'}</pre>
</div>

<style>
  .nc-generada {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .cab {
    display: flex;
    align-items: center;
    gap: 12px;
    justify-content: space-between;
  }
  .cab p {
    margin: 0;
    color: var(--texto-2);
    font-size: 13px;
  }
  pre {
    margin: 0;
    max-height: 560px;
    overflow: auto;
    padding: 14px 16px;
    background: var(--codigo);
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    font: 12.5px/1.55 var(--mono);
    color: var(--texto);
    tab-size: 4;
  }
</style>
