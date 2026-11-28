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

function dePuertos(p) {
  if (p?.tipo === 'lista') return { modo: 'concreto', texto: textoRangos(p.valor), grupo: null };
  if (p?.tipo === 'grupo') return { modo: 'lista', texto: '', grupo: p.valor };
  return { modo: 'cualquiera', texto: '', grupo: null };
}

function aPuertos(p) {
  if (p.modo === 'concreto') {
    const r = leerPuertos(p.texto);
    return r.ok ? { tipo: 'lista', valor: r.valor } : { tipo: 'lista', valor: [] };
  }
  if (p.modo === 'lista') return { tipo: 'grupo', valor: p.grupo };
  return { tipo: 'cualquiera' };
}

function deDir(d, lado) {
  const t = d?.tipo ?? 'cualquiera';
  const vacio = { tipo: 'cualquiera', dir: null, red: '', mac: '' };
  if (t === 'cualquiera') return vacio;
  if (t === 'interfaz') return { ...vacio, tipo: 'red', red: d.iface };
  if (t === 'mac') return { ...vacio, tipo: 'mac', mac: d.mac };
  if (t === 'equipo' && lado === 'origen') return { ...vacio, tipo: 'equipo', dir: d };
  return { ...vacio, tipo: 'ip', dir: d };
}

function aDir(x) {
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

/** Radios del origen; «Red» solo si la zona tiene tarjetas. */
export function opcionesOrigen(tarjetas) {
  return [
    { valor: 'cualquiera', texto: 'Cualquiera' },
    { valor: 'equipo', texto: 'Equipo', ayuda: 'Un equipo de la lista. Si cambia de IP, la regla le sigue (va por su MAC).' },
    ...(tarjetas.length ? [{ valor: 'red', texto: 'Red', ayuda: 'Todo lo que entra por una tarjeta de red de esta zona.' }] : []),
    { valor: 'ip', texto: 'IP', ayuda: 'Una IP, una red (192.168.1.0/24), un rango o un grupo de IPs.' },
    { valor: 'mac', texto: 'MAC', ayuda: 'La dirección física de un equipo, por ejemplo aa:bb:cc:dd:ee:ff.' },
  ];
}

/** Radios del destino (Dominio, País y App llegan en la fase 2.5c). */
export function opcionesDestino(tarjetas) {
  return [
    { valor: 'cualquiera', texto: 'Cualquiera' },
    { valor: 'ip', texto: 'IP', ayuda: 'Un equipo de la lista, una IP, una red, un rango o un grupo de IPs.' },
    ...(tarjetas.length ? [{ valor: 'red', texto: 'Red', ayuda: 'Todo lo que sale por una tarjeta de red de esta zona.' }] : []),
  ];
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
