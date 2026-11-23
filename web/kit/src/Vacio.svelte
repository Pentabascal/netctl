<script>
  import Icono from './Icono.svelte';
  /** @type {{ icono?: string, titulo: string, texto?: string, acciones?: any }} */
  let { icono = 'caja', titulo, texto = '', acciones } = $props();
</script>

<div class="nc-vacio">
  <span class="icono"><Icono nombre={icono} tam={26} trazo={1.5} /></span>
  <h3>{titulo}</h3>
  {#if texto}<p>{texto}</p>{/if}
  {#if acciones}<div class="acciones">{@render acciones()}</div>{/if}
</div>

<style>
  .nc-vacio {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 8px;
    padding: 40px 20px;
    border: 1px dashed var(--borde);
    border-radius: var(--radio);
  }
  .icono {
    width: 56px;
    height: 56px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--superficie-2);
    color: var(--texto-2);
    margin-bottom: 4px;
  }
  h3 {
    font-size: 15px;
  }
  p {
    color: var(--texto-2);
    max-width: 420px;
  }
  .acciones {
    margin-top: 8px;
    display: flex;
    gap: 8px;
  }
</style>
