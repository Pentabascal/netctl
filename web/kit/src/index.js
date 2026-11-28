// Kit visual de netctl. Uso desde la UI de un módulo:
//   import { Tarjeta, Tabla, Chip, Boton, crearApi } from '@netctl/kit';
// La carcasa ya carga `tema.css` (variables CSS globales); los módulos no deben volver a cargarlo.

export { default as Asistente } from './Asistente.svelte';
export { default as Aviso } from './Aviso.svelte';
export { default as Ayuda } from './Ayuda.svelte';
export { default as Barra } from './Barra.svelte';
export { default as Boton } from './Boton.svelte';
export { default as Campo } from './Campo.svelte';
export { default as CampoNumero } from './CampoNumero.svelte';
export { default as Chip } from './Chip.svelte';
export { default as ConfigGenerada } from './ConfigGenerada.svelte';
export { default as Confirmacion60 } from './Confirmacion60.svelte';
export { default as Icono } from './Icono.svelte';
export { default as Interruptor } from './Interruptor.svelte';
export { default as Matriz } from './Matriz.svelte';
export { default as Medidor } from './Medidor.svelte';
export { default as Opcion } from './Opcion.svelte';
export { default as PanelLateral } from './PanelLateral.svelte';
export { default as Pestanas } from './Pestanas.svelte';
export { default as Qr } from './Qr.svelte';
export { default as Registro } from './Registro.svelte';
export { default as SelectorEquipo } from './SelectorEquipo.svelte';
export { default as Tabla } from './Tabla.svelte';
export { default as Tarjeta } from './Tarjeta.svelte';
export { default as Vacio } from './Vacio.svelte';
export {
  crearApi,
  apiNucleo,
  fijarCsrf,
  ErrorApi,
  CABECERA_CSRF,
  EVENTO_SESION_CADUCADA,
  formatoBytes,
  formatoHace,
} from './api.js';
export { ICONOS, svgIcono } from './iconos.js';
export * from './validar.js';
