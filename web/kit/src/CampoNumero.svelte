<script>
  import Campo from './Campo.svelte';
  import { soloDigitos, numeroEnRango, leerPuertos } from './validar.js';
  /**
   * Campo numérico como texto con comprobador (§3b): solo deja escribir cifras y avisa si se
   * sale del rango. Con `puertos`, admite listas y rangos («80, 443, 8000-8100»).
   * `valor` es el texto; `valido` dice si se puede guardar. Vacío vale si no es `requerido`.
   * @type {{ etiqueta?: string, valor?: string, valido?: boolean, min?: number, max?: number,
   *   puertos?: boolean, requerido?: boolean, ayuda?: string, placeholder?: string, sufijo?: string,
   *   mas?: any, error?: string, desactivado?: boolean, enfocar?: boolean }}
   */
  let {
    etiqueta = '',
    valor = $bindable(''),
    valido = $bindable(true),
    min = 1,
    max = 65535,
    puertos = false,
    requerido = false,
    ayuda = '',
    placeholder = '',
    sufijo = '',
    mas,
    error = '',
    desactivado = false,
    enfocar = false,
  } = $props();

  let tocado = $state(false);

  const comprobado = $derived.by(() => {
    const t = String(valor ?? '').trim();
    if (!t) return requerido ? { ok: false, error: 'Este dato hace falta.' } : { ok: true };
    return puertos ? leerPuertos(t) : numeroEnRango(t, min, max);
  });

  $effect(() => {
    valido = comprobado.ok;
  });

  function filtrar(e) {
    const el = e.target;
    const limpio = puertos ? el.value.replace(/[^\d,\- ]/g, '') : soloDigitos(el.value);
    if (limpio !== el.value) {
      el.value = limpio;
      valor = limpio;
    }
  }
</script>

<Campo
  {etiqueta}
  bind:valor
  inputmode={puertos ? 'text' : 'numeric'}
  mono
  {requerido}
  {ayuda}
  {placeholder}
  {sufijo}
  {mas}
  {desactivado}
  {enfocar}
  error={error || (tocado || String(valor ?? '').length > 0 ? (comprobado.ok ? '' : comprobado.error) : '')}
  oninput={filtrar}
  onblur={() => (tocado = true)}
/>
