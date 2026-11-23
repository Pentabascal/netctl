<script>
  /**
   * Campo de formulario con etiqueta, ayuda y error (validación en castellano).
   * tipo: text | password | number | email | textarea | select
   * @type {{ etiqueta?: string, valor?: any, tipo?: string, nombre?: string, placeholder?: string,
   *   ayuda?: string, error?: string, opciones?: {valor: any, texto: string}[], mono?: boolean,
   *   requerido?: boolean, desactivado?: boolean, soloLectura?: boolean, autocompletar?: string,
   *   min?: number, max?: number, filas?: number, enfocar?: boolean, sufijo?: string,
   *   inputmode?: string, maxlength?: number, oninput?: (e: Event) => void, extra?: any }}
   */
  let {
    etiqueta = '',
    valor = $bindable(''),
    tipo = 'text',
    nombre = '',
    placeholder = '',
    ayuda = '',
    error = '',
    opciones = [],
    mono = false,
    requerido = false,
    desactivado = false,
    soloLectura = false,
    autocompletar = 'off',
    min,
    max,
    filas = 4,
    enfocar = false,
    sufijo = '',
    inputmode,
    maxlength,
    oninput,
    extra,
  } = $props();
  const id = 'c-' + Math.random().toString(36).slice(2, 9);

  function foco(el) {
    if (enfocar) queueMicrotask(() => el.focus());
  }
</script>

<div class="nc-campo" class:con-error={!!error}>
  {#if etiqueta}
    <label for={id}>{etiqueta}{#if requerido}<span class="req" aria-hidden="true">*</span>{/if}</label>
  {/if}
  <div class="control" class:mono>
    {#if tipo === 'textarea'}
      <textarea
        {id}
        name={nombre || undefined}
        bind:value={valor}
        {placeholder}
        rows={filas}
        required={requerido}
        disabled={desactivado}
        readonly={soloLectura}
        aria-invalid={!!error}
        {oninput}
        use:foco
      ></textarea>
    {:else if tipo === 'select'}
      <select {id} name={nombre || undefined} bind:value={valor} disabled={desactivado} aria-invalid={!!error} use:foco>
        {#each opciones as o (o.valor)}
          <option value={o.valor}>{o.texto}</option>
        {/each}
      </select>
    {:else}
      <input
        {id}
        name={nombre || undefined}
        type={tipo}
        bind:value={valor}
        {placeholder}
        required={requerido}
        disabled={desactivado}
        readonly={soloLectura}
        autocomplete={autocompletar}
        {min}
        {max}
        {inputmode}
        {maxlength}
        aria-invalid={!!error}
        spellcheck="false"
        {oninput}
        use:foco
      />
    {/if}
    {#if sufijo}<span class="sufijo">{sufijo}</span>{/if}
    {@render extra?.()}
  </div>
  {#if error}
    <p class="error">{error}</p>
  {:else if ayuda}
    <p class="ayuda">{ayuda}</p>
  {/if}
</div>

<style>
  .nc-campo {
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
  .req {
    color: var(--rojo);
    margin-left: 3px;
  }
  .control {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  input,
  select,
  textarea {
    width: 100%;
    min-width: 0;
    height: var(--alto-campo);
    padding: 0 11px;
    background: var(--campo);
    border: 1px solid var(--borde-fuerte);
    border-radius: var(--radio-pq);
    color: var(--texto);
    font: inherit;
    transition: border-color 0.12s, box-shadow 0.12s;
  }
  textarea {
    height: auto;
    padding: 8px 11px;
    resize: vertical;
    line-height: 1.5;
  }
  select {
    appearance: none;
    padding-right: 32px;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='%238c94a0' stroke-width='2' stroke-linecap='round'%3E%3Cpath d='m6 9.5 6 6 6-6'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 10px center;
  }
  .mono input,
  .mono textarea {
    font-family: var(--mono);
    font-size: 13px;
  }
  input::placeholder,
  textarea::placeholder {
    color: var(--texto-3);
  }
  input:hover:not(:disabled):not(:focus),
  select:hover:not(:disabled):not(:focus),
  textarea:hover:not(:disabled):not(:focus) {
    border-color: var(--texto-3);
  }
  input:focus,
  select:focus,
  textarea:focus {
    outline: none;
    border-color: var(--azul);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--azul) 22%, transparent);
  }
  input:disabled,
  select:disabled,
  textarea:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  input[readonly] {
    background: var(--codigo);
    color: var(--texto-2);
  }
  .con-error input,
  .con-error select,
  .con-error textarea {
    border-color: var(--rojo);
  }
  .sufijo {
    position: absolute;
    right: 11px;
    color: var(--texto-3);
    pointer-events: none;
    font-size: 13px;
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
