<script>
  import Ayuda from './Ayuda.svelte';
  /**
   * Opciones en línea como los radios de UniFi («○ Cualquiera ○ Equipo ○ Red»).
   * @type {{ etiqueta?: string, ayuda?: string, ejemplo?: string, valor?: any, desactivado?: boolean,
   *   opciones: { valor: any, texto: string, ayuda?: string, desactivado?: boolean }[], onchange?: (v: any) => void }}
   */
  let { etiqueta = '', ayuda = '', ejemplo = '', valor = $bindable(), opciones, desactivado = false, onchange } = $props();
  const nombre = `nc-radios-${Math.random().toString(36).slice(2)}`;
  function elegir(v) {
    valor = v;
    onchange?.(v);
  }
</script>

<div class="nc-radios" role="radiogroup" aria-label={etiqueta || undefined}>
  {#if etiqueta}
    <span class="etq">{etiqueta}{#if ayuda}<Ayuda texto={ayuda} {ejemplo} titulo="Qué es «{etiqueta}»" />{/if}</span>
  {/if}
  <div class="fila">
    {#each opciones as o (o.valor)}
      <label class:off={desactivado || o.desactivado}>
        <input type="radio" name={nombre} checked={valor === o.valor} disabled={desactivado || o.desactivado} onchange={() => elegir(o.valor)} />
        <span class="punto" aria-hidden="true"></span>
        <span>{o.texto}</span>
        {#if o.ayuda}<Ayuda texto={o.ayuda} titulo="Qué es «{o.texto}»" />{/if}
      </label>
    {/each}
  </div>
</div>

<style>
  .nc-radios {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .etq {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--texto-2);
  }
  .fila {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 18px;
  }
  label {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-height: 24px;
    cursor: pointer;
    font-size: 13px;
  }
  label.off {
    opacity: 0.45;
    cursor: not-allowed;
  }
  input {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
  }
  .punto {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 1px solid var(--texto-3);
    display: grid;
    place-items: center;
    flex: none;
  }
  input:checked + .punto {
    border-color: var(--azul);
    background: radial-gradient(circle, var(--azul) 0 4px, transparent 4.5px);
  }
  input:focus-visible + .punto {
    outline: 2px solid var(--azul);
    outline-offset: 2px;
  }
</style>
