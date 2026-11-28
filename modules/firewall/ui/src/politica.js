// Paso entre el formulario de política (lo que ve el usuario, al estilo UniFi) y el JSON de una
// regla de la API. Sin interfaz: se prueba con `node --test`.
import { leerPuertos } from '../../../../web/kit/src/validar.js';

const DIAS_CORTOS = ['L', 'M', 'X', 'J', 'V', 'S', 'D'];
export const DIAS = ['Lunes', 'Martes', 'Miércoles', 'Jueves', 'Viernes', 'Sábado', 'Domingo'];

const textoRangos = (l) => (l ?? []).map((r) => (r.desde === r.hasta ? `${r.desde}` : `${r.desde}-${r.hasta}`)).join(', ');

/** Regla en blanco con lo recomendado para la pareja de zonas. */
export function nuevaRegla(origen, destino) {
  return {
    nombre: '',
    origen_zona: origen,
    destino_zona: destino,
    accion: destino === 'gateway' || origen === 'externa' ? 'permitir' : 'bloquear',
    protocolo: 'cualquiera',
    numero_protocolo: 0,
    puertos: { tipo: 'cualquiera' },
    puertos_origen: { tipo: 'cualquiera' },
    origen: { tipo: 'cualquiera' },
    destino: { tipo: 'cualquiera' },
    version_ip: 'ambas',
    estado: { tipo: 'todas' },
    horario: { tipo: 'siempre' },
    log: false,
    activa: true,
    descripcion: '',
  };
}

export function dePuertos(p) {
  if (p?.tipo === 'lista') return { modo: 'concreto', texto: textoRangos(p.valor), grupo: null };
  if (p?.tipo === 'grupo') return { modo: 'lista', texto: '', grupo: p.valor };
  return { modo: 'cualquiera', texto: '', grupo: null };
}

export function aPuertos(p) {
  if (p.modo === 'concreto') {
    const r = leerPuertos(p.texto);
    return r.ok ? { tipo: 'lista', valor: r.valor } : { tipo: 'lista', valor: [] };
  }
  if (p.modo === 'lista') return { tipo: 'grupo', valor: p.grupo };
  return { tipo: 'cualquiera' };
}

export function deDir(d, lado) {
  const t = d?.tipo ?? 'cualquiera';
  const vacio = { tipo: 'cualquiera', dir: null, red: '', mac: '', dominios: [], paises: [], apps: [] };
  if (t === 'cualquiera') return vacio;
  if (t === 'dominio') return { ...vacio, tipo: 'dominio', dominios: [...d.nombres] };
  if (t === 'pais') return { ...vacio, tipo: 'pais', paises: [...d.codigos] };
  if (t === 'app') return { ...vacio, tipo: 'app', apps: d.apps.map((id, i) => ({ id, nombre: d.nombres?.[i] ?? `App ${id}` })) };
  if (t === 'interfaz') return { ...vacio, tipo: 'red', red: d.iface };
  if (t === 'mac') return { ...vacio, tipo: 'mac', mac: d.mac };
  if (t === 'equipo' && lado === 'origen') return { ...vacio, tipo: 'equipo', dir: d };
  return { ...vacio, tipo: 'ip', dir: d };
}

export function aDir(x) {
  if (x.tipo === 'dominio') return { tipo: 'dominio', nombres: x.dominios.map((n) => n.trim().toLowerCase()).filter(Boolean) };
  if (x.tipo === 'pais') return { tipo: 'pais', codigos: [...x.paises] };
  if (x.tipo === 'app') return { tipo: 'app', apps: x.apps.map((a) => a.id), nombres: x.apps.map((a) => a.nombre) };
  if (x.tipo === 'red') return { tipo: 'interfaz', iface: x.red };
  if (x.tipo === 'mac') return { tipo: 'mac', mac: x.mac.trim().toLowerCase() };
  if ((x.tipo === 'equipo' || x.tipo === 'ip') && x.dir) return x.dir;
  return { tipo: 'cualquiera' };
}

/** Estado del formulario a partir de una regla. */
export function deRegla(r) {
  const h = r.horario ?? { tipo: 'siempre' };
  const v = h.valor;
  return {
    id: r.id ?? null,
    nombre: r.nombre ?? '',
    descripcion: r.descripcion ?? '',
    activa: r.activa ?? true,
    log: !!r.log,
    accion: r.accion,
    origenZona: r.origen_zona,
    destinoZona: r.destino_zona,
    origen: deDir(r.origen, 'origen'),
    destino: deDir(r.destino, 'destino'),
    puertoOrigen: dePuertos(r.puertos_origen),
    puertoDestino: dePuertos(r.puertos),
    version: r.version_ip ?? 'ambas',
    protocolo: r.protocolo ?? 'cualquiera',
    numero: String(r.numero_protocolo ?? 0),
    estado: r.estado?.tipo ?? 'todas',
    estados: r.estado?.tipo === 'elegir' ? [...r.estado.valor] : ['nueva'],
    horario: h.tipo,
    franja: h.tipo === 'diario' ? { ...v } : h.tipo === 'semanal' ? { ...v.franja } : { desde: '08:00', hasta: '17:00' },
    dias: h.tipo === 'semanal' ? [...v.dias] : [0, 1, 2, 3, 4],
    inicio: h.tipo === 'una_vez' ? v.inicio : null,
    fin: h.tipo === 'una_vez' ? v.fin : null,
    tramos: h.tipo === 'a_medida' ? v.map((t) => ({ dias: [...t.dias], franja: { ...t.franja } })) : [{ dias: [5, 6], franja: { desde: '10:00', hasta: '14:00' } }],
  };
}

function aHorario(f) {
  switch (f.horario) {
    case 'diario': return { tipo: 'diario', valor: { ...f.franja } };
    case 'semanal': return { tipo: 'semanal', valor: { dias: [...f.dias].sort(), franja: { ...f.franja } } };
    case 'una_vez': return { tipo: 'una_vez', valor: { inicio: f.inicio, fin: f.fin } };
    case 'a_medida': return { tipo: 'a_medida', valor: f.tramos.map((t) => ({ dias: [...t.dias].sort(), franja: { ...t.franja } })) };
    default: return { tipo: 'siempre' };
  }
}

/** Regla de la API a partir del formulario. */
export function aRegla(f) {
  const usaPuertos = ['tcp', 'udp', 'tcp_udp'].includes(f.protocolo);
  const r = {
    nombre: f.nombre.trim(),
    origen_zona: f.origenZona,
    destino_zona: f.destinoZona,
    accion: f.accion,
    protocolo: f.protocolo,
    numero_protocolo: f.protocolo === 'otro' ? Number(f.numero) : 0,
    puertos: usaPuertos ? aPuertos(f.puertoDestino) : { tipo: 'cualquiera' },
    puertos_origen: usaPuertos ? aPuertos(f.puertoOrigen) : { tipo: 'cualquiera' },
    origen: aDir(f.origen),
    destino: aDir(f.destino),
    version_ip: f.version,
    estado: f.estado === 'elegir' ? { tipo: 'elegir', valor: [...f.estados] } : { tipo: f.estado },
    horario: aHorario(f),
    log: f.log,
    activa: f.activa,
    descripcion: f.descripcion.trim(),
  };
  return f.id != null ? { id: f.id, ...r } : r;
}

const PAIS = { valor: 'pais', texto: 'País', ayuda: 'Todo el tráfico de las IPs de esos países (según la base libre de DB-IP, que se actualiza cada mes).' };

/** Radios del origen; «Red» solo si la zona tiene tarjetas y «País» solo desde Externa. */
export function opcionesOrigen(tarjetas, { externa = false } = {}) {
  return [
    { valor: 'cualquiera', texto: 'Cualquiera' },
    { valor: 'equipo', texto: 'Equipo', ayuda: 'Un equipo de la lista. Si cambia de IP, la regla le sigue (va por su MAC).' },
    ...(tarjetas.length ? [{ valor: 'red', texto: 'Red', ayuda: 'Todo lo que entra por una tarjeta de red de esta zona.' }] : []),
    { valor: 'ip', texto: 'IP', ayuda: 'Una IP, una red (192.168.1.0/24), un rango o un grupo de IPs.' },
    { valor: 'mac', texto: 'MAC', ayuda: 'La dirección física de un equipo, por ejemplo aa:bb:cc:dd:ee:ff.' },
    ...(externa ? [PAIS] : []),
  ];
}

/**
 * Radios del destino. Hacia internet (zona Externa) también Dominio, País y App; App sale
 * desactivada con el motivo si `apps` (GET /apps) dice que no se puede.
 */
export function opcionesDestino(tarjetas, { externa = false, apps = null } = {}) {
  const app = { valor: 'app', texto: 'App', ayuda: 'Aplicaciones que se reconocen por su tráfico (YouTube, TikTok, Netflix…) con nDPI. Se detectan en los primeros paquetes de cada conexión.' };
  if (apps && apps.disponible === false) {
    app.desactivado = true;
    app.ayuda = apps.motivo;
  }
  return [
    { valor: 'cualquiera', texto: 'Cualquiera' },
    { valor: 'ip', texto: 'IP', ayuda: 'Un equipo de la lista, una IP, una red, un rango o un grupo de IPs.' },
    ...(tarjetas.length ? [{ valor: 'red', texto: 'Red', ayuda: 'Todo lo que sale por una tarjeta de red de esta zona.' }] : []),
    ...(externa
      ? [{ valor: 'dominio', texto: 'Dominio', ayuda: 'Nombres como tiktok.com. Se preguntan al DNS cada 5 minutos y se bloquean las IPs que da.' }, PAIS, app]
      : []),
  ];
}

/** ¿Es un nombre de dominio válido? (como el servidor: etiquetas de 1 a 63, ≤ 253) */
export function esDominio(s) {
  const t = (s ?? '').trim().toLowerCase();
  return t.length > 0 && t.length <= 253 && t.split('.').every((e) => /^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$/.test(e));
}

/** Bandera emoji de un código de país. */
export function bandera(cc) {
  return String.fromCodePoint(...[...cc.toUpperCase()].map((c) => 0x1f1e6 + c.charCodeAt(0) - 65));
}

const dias = (d) => [...d].sort().map((x) => DIAS_CORTOS[x]).join(', ');
const franja = (f) => `${f.desde}–${f.hasta}`;

/** «L, V 08:00–17:00» */
export function textoHorario(h) {
  switch (h?.tipo) {
    case 'diario': return `Cada día ${franja(h.valor)}`;
    case 'semanal': return `${dias(h.valor.dias)} ${franja(h.valor.franja)}`;
    case 'una_vez': return `Del ${new Date(h.valor.inicio * 1000).toLocaleString('es')} al ${new Date(h.valor.fin * 1000).toLocaleString('es')}`;
    case 'a_medida': return h.valor.map((t) => `${dias(t.dias)} ${franja(t.franja)}`).join(' · ');
    default: return 'Siempre';
  }
}
