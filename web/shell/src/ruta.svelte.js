// Enrutado mínimo por la ruta del navegador (el núcleo devuelve index.html para cualquier ruta).
// Las rutas que existen están en rutas.js.
//   /                     Inicio
//   /modulos              Módulos
//   /ajustes              Ajustes
//   /modulo/<id>/<ruta>   Página de un módulo (entrada de su menú)

export const ruta = $state({ actual: location.pathname });

export function navegar(destino, reemplazar = false) {
  if (destino === ruta.actual) return;
  if (reemplazar) history.replaceState(null, '', destino);
  else history.pushState(null, '', destino);
  ruta.actual = location.pathname;
  window.scrollTo(0, 0);
}

addEventListener('popstate', () => {
  ruta.actual = location.pathname;
});

// Los enlaces internos (<a href="/...">) no recargan la página.
addEventListener('click', (e) => {
  if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
  const a = e.target.closest?.('a[href]');
  if (!a || a.target || a.hasAttribute('download')) return;
  const url = new URL(a.href, location.href);
  if (url.origin !== location.origin) return;
  if (url.pathname.startsWith('/api/') || url.pathname.startsWith('/m/')) return;
  e.preventDefault();
  navegar(url.pathname + url.search);
});

export { analizar } from './rutas.js';
