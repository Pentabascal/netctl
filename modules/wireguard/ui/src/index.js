// Punto de entrada que carga la carcasa: mount(el, api, { modulo, ruta, navegar }) -> desmontar.
import { mount as montar, unmount } from 'svelte';
import App from './App.svelte';

export function mount(el, api, ctx = {}) {
  const app = montar(App, {
    target: el,
    props: { api, ruta: ctx.ruta ?? '', navegar: ctx.navegar ?? (() => {}) },
  });
  return () => unmount(app);
}
