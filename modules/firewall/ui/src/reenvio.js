// Paso entre el formulario de un reenvío (lo que ve el usuario) y el JSON de la API.

const DURACION = { '1h': 3600, '1d': 86400, '1s': 7 * 86400 };

export const CADUCA = [
  { valor: 'nunca', texto: 'Nunca (recomendado)' },
  { valor: '1h', texto: 'Dentro de 1 hora' },
  { valor: '1d', texto: 'Dentro de 1 día' },
  { valor: '1s', texto: 'Dentro de 1 semana' },
];

function rangos(texto) {
  return texto
    .split(',')
    .map((x) => x.trim())
    .filter(Boolean)
    .map((p) => {
      const [a, b = a] = p.split('-').map(Number);
      return { desde: a, hasta: b };
    });
}

export function formNuevo() {
  return {
    nombre: '',
    equipo: null,
    plantilla: '',
    puertos: '',
    protocolo: 'tcp',
    fuera: '',
    soloAlgunas: false,
    desde: [],
    caduca: 'nunca',
    caducaActual: null,
    wan: '',
    loopback: true,
    log: false,
    activo: true,
  };
}

export function deReenvio(r) {
  const unico = r.externo.length === 1 && r.externo[0].desde === r.externo[0].hasta;
  return {
    nombre: r.nombre,
    equipo: r.equipo,
    plantilla: r.plantilla ?? 'otro',
    puertos: r.interno != null ? String(r.interno) : r.externo.map((x) => (x.desde === x.hasta ? `${x.desde}` : `${x.desde}-${x.hasta}`)).join(', '),
    protocolo: r.protocolo,
    fuera: r.interno != null && unico ? String(r.externo[0].desde) : '',
    soloAlgunas: r.desde.length > 0,
    desde: r.desde,
    caduca: r.caduca ? 'mantener' : 'nunca',
    caducaActual: r.caduca ?? null,
    wan: r.wan ?? '',
    loopback: r.loopback,
    log: r.log,
    activo: r.activo,
  };
}

/** ¿Hay un único puerto (no lista ni rango)? Solo entonces vale un puerto de fuera distinto. */
export function puertoUnico(texto) {
  const l = rangos(texto);
  return l.length === 1 && l[0].desde === l[0].hasta ? l[0].desde : null;
}

/** JSON para la API. `nombreServicio`: para el nombre por defecto. */
export function aReenvio(f, nombreServicio = '', ahora = Date.now() / 1000) {
  const unico = puertoUnico(f.puertos);
  const fuera = Number(f.fuera);
  let externo = rangos(f.puertos);
  let interno = null;
  if (unico != null && f.fuera.trim() && fuera !== unico) {
    externo = [{ desde: fuera, hasta: fuera }];
    interno = unico;
  }
  const equipoNombre = f.equipo?.nombre || (f.equipo?.red ?? '').replace(/\/32$/, '') || f.equipo?.ip || '';
  const servicio = nombreServicio || `Puerto ${f.puertos}`;
  let caduca = null;
  if (f.caduca === 'mantener') caduca = f.caducaActual;
  else if (DURACION[f.caduca]) caduca = Math.round(ahora + DURACION[f.caduca]);
  return {
    nombre: f.nombre.trim() || `${equipoNombre} · ${servicio}`,
    equipo: f.equipo,
    protocolo: f.protocolo,
    externo,
    interno,
    wan: f.wan || null,
    desde: f.soloAlgunas ? f.desde : [],
    caduca,
    loopback: f.loopback,
    activo: f.activo,
    log: f.log,
    plantilla: f.plantilla || null,
  };
}
