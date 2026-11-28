// Ruta del navegador → vista (puro, para poder probarlo con `node --test`).
//   /                          Inicio
//   /modulos                   Módulos
//   /ajustes/<seccion>         Ajustes del núcleo
//   /ajustes/m/<id>/<ruta>     Página de Ajustes que aporta un módulo ([[ajustes]])
//   /modulo/<id>/<ruta>        Página de un módulo (entrada de su menú)
export function analizar(p) {
  const partes = p.split('/').filter(Boolean);
  if (partes.length === 0) return { pagina: 'inicio' };
  if (partes[0] === 'modulos') return { pagina: 'modulos' };
  if (partes[0] === 'ajustes' && partes[1] === 'm' && partes[2]) {
    return { pagina: 'ajustes', seccion: 'm', id: partes[2], sub: partes.slice(3).join('/') };
  }
  if (partes[0] === 'ajustes') return { pagina: 'ajustes', seccion: partes[1] };
  if (partes[0] === 'modulo' && partes[1]) return { pagina: 'modulo', id: partes[1], sub: partes.slice(2).join('/') };
  return { pagina: 'no-existe' };
}
