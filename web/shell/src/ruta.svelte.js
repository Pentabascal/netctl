// Enrutado mínimo por la ruta del navegador (el núcleo devuelve index.html para cualquier ruta).
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

export function analizar(p) {
  const partes = p.split('/').filter(Boolean);
  if (partes.length === 0) return { pagina: 'inicio' };
  if (partes[0] === 'modulos') return { pagina: 'modulos' };
  if (partes[0] === 'ajustes') return { pagina: 'ajustes', seccion: partes[1] };
  if (partes[0] === 'modulo' && partes[1]) return { pagina: 'modulo', id: partes[1], sub: partes.slice(2).join('/') };
  return { pagina: 'no-existe' };
}
