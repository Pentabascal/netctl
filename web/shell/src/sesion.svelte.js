// Estado compartido de la carcasa: usuario, módulos instalados (para el menú) y versión.
import { apiNucleo, fijarCsrf } from '@netctl/kit/api.js';

export const sesion = $state({
  cargando: true,
  yo: null, // { usuario, csrf, totp_activo, alcance, version }
  modulos: [], // GET /api/modulos
  aviso: '', // mensaje para el login (p. ej. «contraseña cambiada»)
});

export async function cargarYo() {
  try {
    const yo = await apiNucleo.get('/api/yo');
    fijarCsrf(yo.csrf);
    sesion.yo = yo;
    await cargarModulos();
  } catch {
    sesion.yo = null;
  } finally {
    sesion.cargando = false;
  }
}

export async function cargarModulos() {
  try {
    const r = await apiNucleo.get('/api/modulos');
    sesion.modulos = r.modulos ?? [];
  } catch {
    /* sin módulos o sin sesión */
  }
}

export function cerrarLocal(aviso = '') {
  fijarCsrf(null);
  sesion.yo = null;
  sesion.modulos = [];
  sesion.aviso = aviso;
}

export async function salir() {
  try {
    await apiNucleo.post('/api/logout');
  } catch {
    /* da igual */
  }
  cerrarLocal();
}
