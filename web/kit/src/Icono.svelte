<script>
  import { svgIcono } from './iconos.js';
  /** @type {{ nombre: string, tam?: number, trazo?: number, clase?: string }} */
  let { nombre, tam = 18, trazo = 1.75, clase = '' } = $props();
</script>

<svg
  class="nc-icono {clase}"
  width={tam}
  height={tam}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={trazo}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true">{@html svgIcono(nombre)}</svg>

<style>
  .nc-icono {
    flex: none;
    display: block;
  }
</style>
