// Cliente de la API de netctl. La sesión va en la cookie `netctl_sesion` (HttpOnly) y las
// peticiones que cambian algo llevan la cabecera `X-Netctl-Csrf`, que se lee de `GET /api/yo`.
//
//   const api = crearApi('/api/m/wireguard');
//   const datos = await api.get('/clientes');
//   await api.post('/clientes', { nombre: 'Portátil' });
//   const cerrar = api.sse('/vivo', (evento) => { ... });
//
// Los errores se lanzan como `ErrorApi` con `estado` (código HTTP), `mensaje` (en castellano,
// listo para mostrar) y `datos` (el JSON de la respuesta, p. ej. `totp_requerido`).
// Un 401 en una ruta que no sea el login emite el evento `netctl:sesion-caducada` en `window`.

export const CABECERA_CSRF = 'X-Netctl-Csrf';
export const EVENTO_SESION_CADUCADA = 'netctl:sesion-caducada';

let csrfGlobal = null;
let csrfPendiente = null;

/** Fija el token CSRF compartido (la carcasa lo hace tras el login o `GET /api/yo`). */
export function fijarCsrf(token) {
  csrfGlobal = token || null;
}

async function obtenerCsrf() {
  if (csrfGlobal) return csrfGlobal;
  if (!csrfPendiente) {
    csrfPendiente = fetch('/api/yo', { credentials: 'same-origin', headers: { Accept: 'application/json' } })
      .then((r) => (r.ok ? r.json() : null))
      .then((j) => {
        if (j?.csrf) csrfGlobal = j.csrf;
        return csrfGlobal;
      })
      .finally(() => {
        csrfPendiente = null;
      });
  }
  return csrfPendiente;
}

export class ErrorApi extends Error {
  constructor(estado, mensaje, datos = null) {
    super(mensaje);
    this.name = 'ErrorApi';
    this.estado = estado;
    this.mensaje = mensaje;
    this.datos = datos;
  }
}

function mensajePorDefecto(estado) {
  if (estado === 0) return 'No se pudo contactar con el panel. Comprueba la conexión.';
  if (estado === 401) return 'La sesión ha caducado. Vuelve a iniciar sesión.';
  if (estado === 403) return 'No tienes permiso para hacer esto.';
  if (estado === 404) return 'No se ha encontrado.';
  if (estado === 409) return 'Alguien ha cambiado esto a la vez. Recarga y vuelve a intentarlo.';
  if (estado === 422) return 'Hay datos que no son válidos.';
  if (estado === 502 || estado === 503) return 'El servicio no responde ahora mismo.';
  return `Error inesperado (${estado}).`;
}

function unir(base, ruta) {
  if (/^https?:\/\//.test(ruta)) return ruta;
  const b = base.endsWith('/') ? base.slice(0, -1) : base;
  if (!ruta) return b || '/';
  return b + (ruta.startsWith('/') ? ruta : '/' + ruta);
}

/**
 * Crea un cliente para una base de rutas (`''` para el núcleo, `/api/m/<id>` para un módulo).
 * @param {string} base
 */
export function crearApi(base = '') {
  async function peticion(metodo, ruta, cuerpo, opciones = {}) {
    const cabeceras = { Accept: 'application/json', ...(opciones.cabeceras || {}) };
    let body;
    if (cuerpo instanceof FormData || cuerpo instanceof Blob || typeof cuerpo === 'string') {
      body = cuerpo;
    } else if (cuerpo !== undefined) {
      body = JSON.stringify(cuerpo);
      cabeceras['Content-Type'] = 'application/json';
    }
    if (metodo !== 'GET' && metodo !== 'HEAD') {
      const t = await obtenerCsrf();
      if (t) cabeceras[CABECERA_CSRF] = t;
    }
    const url = unir(base, ruta);
    let r;
    try {
      r = await fetch(url, { method: metodo, headers: cabeceras, body, credentials: 'same-origin', signal: opciones.signal });
    } catch (e) {
      if (e?.name === 'AbortError') throw e;
      throw new ErrorApi(0, mensajePorDefecto(0));
    }
    const tipo = r.headers.get('content-type') || '';
    let datos = null;
    if (r.status !== 204) {
      if (tipo.includes('application/json')) datos = await r.json().catch(() => null);
      else if (opciones.texto || !r.ok) datos = await r.text().catch(() => null);
      else datos = await r.blob();
    }
    if (!r.ok) {
      const mensaje = (datos && typeof datos === 'object' && datos.error) || mensajePorDefecto(r.status);
      if (r.status === 401 && !url.endsWith('/api/login') && typeof window !== 'undefined') {
        csrfGlobal = null;
        window.dispatchEvent(new CustomEvent(EVENTO_SESION_CADUCADA));
      }
      throw new ErrorApi(r.status, mensaje, typeof datos === 'object' ? datos : null);
    }
    return datos;
  }

  return {
    base,
    /** URL absoluta de una ruta (para `href` de descargas, `<img src>`, etc.). */
    url: (ruta) => unir(base, ruta),
    get: (ruta, opciones) => peticion('GET', ruta, undefined, opciones),
    post: (ruta, cuerpo, opciones) => peticion('POST', ruta, cuerpo, opciones),
    put: (ruta, cuerpo, opciones) => peticion('PUT', ruta, cuerpo, opciones),
    patch: (ruta, cuerpo, opciones) => peticion('PATCH', ruta, cuerpo, opciones),
    del: (ruta, cuerpo, opciones) => peticion('DELETE', ruta, cuerpo, opciones),
    /**
     * Se suscribe a un flujo SSE. `cb` recibe cada mensaje ya parseado como JSON (o el texto
     * si no lo es). Reconecta solo (EventSource). Devuelve una función para cerrarlo.
     */
    sse(ruta, cb, alError) {
      const es = new EventSource(unir(base, ruta), { withCredentials: true });
      es.onmessage = (ev) => {
        let d = ev.data;
        try {
          d = JSON.parse(ev.data);
        } catch {
          /* texto plano */
        }
        cb(d, ev);
      };
      if (alError) es.onerror = alError;
      return () => es.close();
    },
  };
}

/** Cliente del núcleo (`/api/...`). */
export const apiNucleo = crearApi('');

// ---------------------------------------------------------------- formato

/** Bytes a texto legible: 1536 → «1,5 KB». */
export function formatoBytes(n) {
  if (n == null || isNaN(n)) return '—';
  const u = ['B', 'KB', 'MB', 'GB', 'TB'];
  let i = 0;
  let v = Number(n);
  while (v >= 1024 && i < u.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toLocaleString('es-ES', { maximumFractionDigits: v < 10 && i > 0 ? 1 : 0 })} ${u[i]}`;
}

/** Segundos desde un instante UNIX a texto: «hace 3 min». `null`/`0` → «Nunca». */
export function formatoHace(unix, ahora = Date.now() / 1000) {
  if (!unix) return 'Nunca';
  const s = Math.max(0, Math.round(ahora - unix));
  if (s < 10) return 'ahora mismo';
  if (s < 60) return `hace ${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `hace ${m} min`;
  const h = Math.floor(m / 60);
  if (h < 24) return `hace ${h} h`;
  const d = Math.floor(h / 24);
  return `hace ${d} ${d === 1 ? 'día' : 'días'}`;
}
