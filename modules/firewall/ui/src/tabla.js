// Lógica sin interfaz de la Tabla de políticas (filtros con contador, como UniFi) y del panel de
// NAT. Se prueba con `node --test`.
import { dePuertos, aPuertos, deDir, aDir } from './politica.js';

/** Dimensiones de los filtros de la izquierda (además de «Tipo»). */
export const DIMENSIONES = [
  { id: 'accion', titulo: 'Acción' },
  { id: 'protocolo', titulo: 'Protocolo' },
  { id: 'origen', titulo: 'Origen' },
  { id: 'destino', titulo: 'Destino' },
];

export const TIPOS = [
  { valor: 'firewall', texto: 'Firewall' },
  { valor: 'nat', texto: 'NAT' },
  { valor: 'reenvio', texto: 'Reenvío de puertos' },
];

/** Tipos para el bloque «Tipo» del panel. Con una sola tarjeta no hay a quién reenviar. */
export function tiposDisponibles(unaInterfaz) {
  return TIPOS.map((t) =>
    t.valor === 'reenvio' && unaInterfaz ? { ...t, desactivado: true, ayuda: 'Esta máquina tiene una sola tarjeta: no es el router de la red, así que no puede reenviar a otros equipos.' } : t,
  );
}

/** Columnas de la tabla; `porDefecto` las que se ven al principio. */
export const COLUMNAS_TABLA = [
  { id: 'nombre', titulo: 'Nombre', porDefecto: true },
  { id: 'tipo', titulo: 'Tipo', porDefecto: true },
  { id: 'accion', titulo: 'Acción', porDefecto: true },
  { id: 'protocolo', titulo: 'Protocolo', porDefecto: true },
  { id: 'zona_origen', titulo: 'Zona origen', porDefecto: true },
  { id: 'origen', titulo: 'Origen', porDefecto: true },
  { id: 'zona_destino', titulo: 'Zona destino', porDefecto: true },
  { id: 'destino', titulo: 'Destino', porDefecto: true },
  { id: 'puerto_destino', titulo: 'Puerto destino', porDefecto: true },
  { id: 'interfaz', titulo: 'Interfaz', porDefecto: false },
  { id: 'estado', titulo: 'Estado', porDefecto: true },
];

export function filtrosVacios() {
  return { tipo: 'todas', accion: [], protocolo: [], origen: [], destino: [], serie: false, texto: '' };
}

/** Cuántas filas hay por cada valor de cada filtro (las pausadas también cuentan). */
export function contadores(filas) {
  const c = { tipo: { firewall: 0, nat: 0, reenvio: 0 } };
  for (const d of DIMENSIONES) c[d.id] = {};
  for (const f of filas) {
    c.tipo[f.tipo] = (c.tipo[f.tipo] ?? 0) + 1;
    for (const d of DIMENSIONES) {
      const v = f[d.id] ?? '—';
      c[d.id][v] = (c[d.id][v] ?? 0) + 1;
    }
  }
  return c;
}

const sinAcentos = (s) =>
  (s ?? '')
    .toString()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase();

/** Filas que se ven: dentro de un filtro vale cualquiera de lo marcado; entre filtros, todos. */
export function aplicarFiltros(filas, f) {
  const q = sinAcentos(f.texto).trim();
  return filas.filter((x) => {
    if (x.de_serie && !f.serie) return false;
    if (f.tipo !== 'todas' && x.tipo !== f.tipo) return false;
    for (const d of DIMENSIONES) {
      if (f[d.id].length && !f[d.id].includes(x[d.id] ?? '—')) return false;
    }
    if (!q) return true;
    return [x.nombre, x.origen, x.destino, x.puerto_destino, x.zona_origen, x.zona_destino, x.interfaz].some((c) => sinAcentos(c).includes(q));
  });
}

// ---------------------------------------------------------------- NAT

export function nuevaNat() {
  return {
    nombre: '',
    tipo: { tipo: 'masquerade' },
    iface: null,
    origen: { tipo: 'cualquiera' },
    destino: { tipo: 'cualquiera' },
    protocolo: 'cualquiera',
    puertos: { tipo: 'cualquiera' },
    activa: true,
    descripcion: '',
  };
}

/** Formulario a partir de una regla NAT de la API. */
export function deNat(n) {
  return {
    nombre: n.nombre ?? '',
    tipo: n.tipo.tipo,
    ip: n.tipo.ip ?? '',
    puertoNuevo: n.tipo.puerto != null ? String(n.tipo.puerto) : '',
    iface: n.iface ?? '',
    origen: deDir(n.origen, 'destino'),
    destino: deDir(n.destino, 'destino'),
    protocolo: n.protocolo ?? 'cualquiera',
    puertos: dePuertos(n.puertos),
    activa: n.activa ?? true,
    descripcion: n.descripcion ?? '',
  };
}

/** JSON para la API (sin `id`). */
export function aNat(f) {
  const ip = f.ip.trim();
  let tipo = { tipo: 'masquerade' };
  if (f.tipo === 'snat') tipo = { tipo: 'snat', ip };
  if (f.tipo === 'dnat') tipo = { tipo: 'dnat', ip, puerto: f.puertoNuevo.trim() ? Number(f.puertoNuevo) : null };
  const usaPuertos = ['tcp', 'udp', 'tcp_udp'].includes(f.protocolo);
  return {
    nombre: f.nombre.trim(),
    tipo,
    iface: f.iface || null,
    origen: aDir(f.origen),
    destino: aDir(f.destino),
    protocolo: f.protocolo,
    puertos: usaPuertos ? aPuertos(f.puertos) : { tipo: 'cualquiera' },
    activa: f.activa,
    descripcion: f.descripcion,
  };
}
