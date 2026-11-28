// Lógica sin interfaz del kit (se prueba con `node --test web/kit/src/`).
// Las direcciones tienen la misma forma JSON que el modelo de los módulos:
//   { tipo: 'cualquiera' } · { tipo: 'equipo', mac, ip, nombre } · { tipo: 'ip', red: 'a.b.c.d/n' }
//   { tipo: 'rango', desde, hasta } · { tipo: 'grupo', id }

const RE_V4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/;

function v4(texto) {
  const m = RE_V4.exec(texto);
  if (!m) return null;
  const o = m.slice(1).map(Number);
  if (o.some((x) => x > 255)) return null;
  return ((o[0] << 24) >>> 0) + (o[1] << 16) + (o[2] << 8) + o[3];
}

/** Expande una IPv6 a 8 grupos numéricos; null si no es válida. */
function v6(texto) {
  if (!/^[0-9a-fA-F:]+$/.test(texto) || !texto.includes(':')) return null;
  const partes = texto.split('::');
  if (partes.length > 2) return null;
  const izq = partes[0] ? partes[0].split(':') : [];
  const der = partes.length === 2 && partes[1] ? partes[1].split(':') : [];
  const faltan = 8 - izq.length - der.length;
  if ((partes.length === 1 && faltan !== 0) || faltan < 0) return null;
  const grupos = [...izq, ...Array(partes.length === 2 ? faltan : 0).fill('0'), ...der];
  if (grupos.length !== 8 || grupos.some((g) => g.length === 0 || g.length > 4)) return null;
  return grupos.map((g) => parseInt(g, 16));
}

function familia(ip) {
  if (v4(ip) !== null) return 4;
  if (v6(ip) !== null) return 6;
  return 0;
}

function comparar(a, b) {
  if (familia(a) === 4) return v4(a) - v4(b);
  const x = v6(a);
  const y = v6(b);
  for (let i = 0; i < 8; i++) if (x[i] !== y[i]) return x[i] - y[i];
  return 0;
}

/** ¿Está la IPv4 `ip` dentro de `cidr`? (IPv6: false) */
export function ipEnRed(ip, cidr) {
  const [red, n] = cidr.split('/');
  const a = v4(ip);
  const b = v4(red);
  const p = Number(n);
  if (a === null || b === null || !(p >= 0 && p <= 32)) return false;
  if (p === 0) return true;
  const mascara = (~0 << (32 - p)) >>> 0;
  return ((a & mascara) >>> 0) === ((b & mascara) >>> 0);
}

/**
 * Lee lo escrito a mano en un selector de equipos.
 * opciones: { redes: admite CIDR y rangos, propias: IPs de esta máquina, conocidas: CIDR de sus redes }
 * → { ok: true, dir, aviso? } | { ok: false, error }
 */
export function leerEntrada(texto, { redes = false, propias = [], conocidas = [] } = {}) {
  const t = (texto ?? '').trim();
  if (!t) return { ok: false, error: 'Escribe una IP o elige un equipo de la lista.' };
  if (t.includes('-')) {
    if (!redes) return { ok: false, error: 'Aquí vale solo una IP, no un rango.' };
    const [a, b] = t.split('-').map((x) => x.trim());
    const fa = familia(a);
    const fb = familia(b);
    if (!fa || !fb) return { ok: false, error: `«${t}» no es un rango válido. Ejemplo: 192.168.1.10-192.168.1.20` };
    if (fa !== fb) return { ok: false, error: 'Un rango no puede mezclar IPv4 e IPv6.' };
    if (comparar(a, b) > 0) return { ok: false, error: 'El rango está al revés: escribe primero la IP menor.' };
    return { ok: true, dir: { tipo: 'rango', desde: a, hasta: b } };
  }
  if (t.includes('/')) {
    if (!redes) return { ok: false, error: 'Aquí vale solo una IP, no una red entera.' };
    const [ip, n] = t.split('/');
    const f = familia(ip);
    const p = Number(n);
    const max = f === 4 ? 32 : 128;
    if (!f || !/^\d+$/.test(n) || p > max) {
      return { ok: false, error: `«${t}» no es una red válida. Ejemplo: 192.168.1.0/24` };
    }
    return { ok: true, dir: { tipo: 'ip', red: `${ip}/${p}` } };
  }
  const f = familia(t);
  if (!f) {
    return { ok: false, error: `«${t}» no es una IP válida. Ejemplo: 192.168.1.50` };
  }
  if (propias.includes(t)) {
    return { ok: false, error: 'Esa es la IP de esta máquina. Elige otro equipo.' };
  }
  const r = { ok: true, dir: { tipo: 'ip', red: `${t}/${f === 4 ? 32 : 128}` } };
  if (f === 4 && conocidas.length && !conocidas.some((c) => ipEnRed(t, c))) {
    r.aviso = 'Esa IP no está en ninguna red de esta máquina: comprueba que esté bien escrita.';
  }
  return r;
}

/** «80, 443, 8000-8100» → { ok, valor: [{desde, hasta}] } | { ok: false, error } */
export function leerPuertos(texto) {
  const partes = (texto ?? '')
    .split(',')
    .map((x) => x.trim())
    .filter(Boolean);
  if (!partes.length) return { ok: false, error: 'Escribe al menos un puerto, por ejemplo 80 o 8000-8100.' };
  const uno = (x) => (/^\d+$/.test(x.trim()) && +x >= 1 && +x <= 65535 ? +x : null);
  const valor = [];
  for (const p of partes) {
    const [a, b = a] = p.split('-');
    const d = uno(a);
    const h = uno(b);
    if (d === null || h === null) return { ok: false, error: `«${p}» no es un puerto válido: usa números del 1 al 65535.` };
    if (d > h) return { ok: false, error: `El rango ${p} está al revés: escribe primero el número menor (${h}-${d}).` };
    valor.push({ desde: d, hasta: h });
  }
  return { ok: true, valor };
}

export function soloDigitos(texto) {
  return (texto ?? '').replace(/\D/g, '');
}

export function numeroEnRango(texto, min, max) {
  const t = (texto ?? '').trim();
  if (!/^\d+$/.test(t)) return { ok: false, error: `Escribe un número entre ${min} y ${max}.` };
  const n = Number(t);
  if (n < min || n > max) return { ok: false, error: `Tiene que estar entre ${min} y ${max}.` };
  return { ok: true, valor: n };
}

/** Texto corto de una dirección. `grupos`: { id: nombre } */
export function textoDireccion(d, grupos = {}) {
  if (!d || d.tipo === 'cualquiera') return 'Cualquiera';
  if (d.tipo === 'equipo') return d.nombre ? `${d.nombre} · ${d.ip}` : d.ip;
  if (d.tipo === 'ip') return d.red.replace(/\/(32|128)$/, '');
  if (d.tipo === 'rango') return `${d.desde} – ${d.hasta}`;
  if (d.tipo === 'grupo') return `Grupo «${grupos[d.id] ?? d.id}»`;
  return '';
}

const sinAcentos = (s) =>
  (s ?? '')
    .toString()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase();

/** Filtra por nombre, IP, MAC o fabricante (sin distinguir acentos ni mayúsculas). */
export function buscarEquipos(lista, q) {
  const t = sinAcentos(q).trim();
  if (!t) return lista;
  return lista.filter((e) => [e.nombre, e.ip, e.mac, e.fabricante].some((c) => sinAcentos(c).includes(t)));
}

/** [{ red, equipos }] con los conectados primero dentro de cada red; redes en orden de aparición. */
export function agruparEquipos(lista) {
  const grupos = [];
  for (const e of lista) {
    let g = grupos.find((x) => x.red === e.red);
    if (!g) grupos.push((g = { red: e.red, equipos: [] }));
    g.equipos.push(e);
  }
  for (const g of grupos) g.equipos.sort((a, b) => Number(!!b.conectado) - Number(!!a.conectado));
  return grupos;
}
