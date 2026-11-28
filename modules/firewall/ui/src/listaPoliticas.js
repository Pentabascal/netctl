// Lógica sin interfaz de la página Zonas (se prueba con `node --test`).

/** Orden de UniFi en la matriz y el esquema; las personalizadas van al final. */
export const ORDEN_ZONAS = ['interna', 'externa', 'gateway', 'vpn', 'hotspot', 'dmz'];

export function ordenarZonas(zonas) {
  const pos = (z) => {
    const i = ORDEN_ZONAS.indexOf(z.tipo);
    return i < 0 ? ORDEN_ZONAS.length : i;
  };
  return [...zonas].sort((a, b) => pos(a) - pos(b));
}

/** Columnas de la lista de políticas (spec §3.6). */
export const COLUMNAS = [
  { id: 'nombre', titulo: 'Nombre', porDefecto: true },
  { id: 'accion', titulo: 'Acción', porDefecto: true },
  { id: 'version_ip', titulo: 'Versión IP', porDefecto: true },
  { id: 'protocolo', titulo: 'Protocolo', porDefecto: true },
  { id: 'zona_origen', titulo: 'Zona origen', porDefecto: true },
  { id: 'origen', titulo: 'Origen', porDefecto: true },
  { id: 'puerto_origen', titulo: 'Puerto origen', porDefecto: true },
  { id: 'zona_destino', titulo: 'Zona destino', porDefecto: true },
  { id: 'destino', titulo: 'Destino', porDefecto: true },
  { id: 'puerto_destino', titulo: 'Puerto destino', porDefecto: true },
];

const sinAcentos = (s) =>
  (s ?? '')
    .toString()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase();

/**
 * Políticas propias que se ven con los filtros de la página.
 * `par`: [origen, destino] de la casilla elegida o null. `v4`/`v6`: casillas de versión IP
 * («Ambas» se ve con cualquiera de las dos marcada). `texto`: buscador.
 */
export function filtrar(propias, { par = null, v4 = true, v6 = true, texto = '' } = {}) {
  const q = sinAcentos(texto).trim();
  return propias.filter((r) => {
    if (par && (r.origen_zona !== par[0] || r.destino_zona !== par[1])) return false;
    const vi = r.version_ip ?? 'ambas';
    if (vi === 'v4' && !v4) return false;
    if (vi === 'v6' && !v6) return false;
    if (vi === 'ambas' && !v4 && !v6) return false;
    if (!q) return true;
    return [r.nombre, r.descripcion, r.texto_origen, r.texto_destino, r.texto_puertos, r.zona_origen_nombre, r.zona_destino_nombre]
      .some((c) => sinAcentos(c).includes(q));
  });
}

/** Sube (`delta` -1) o baja (+1) `id` en la lista de ids, sin salirse. */
export function mover(ids, id, delta) {
  const l = [...ids];
  const i = l.indexOf(id);
  const j = i + delta;
  if (i < 0 || j < 0 || j >= l.length) return l;
  [l[i], l[j]] = [l[j], l[i]];
  return l;
}
