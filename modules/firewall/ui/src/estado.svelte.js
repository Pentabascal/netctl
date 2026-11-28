// Estado compartido de la interfaz del Firewall: el cambio a la espera de confirmación y un
// contador que sube cada vez que algo cambia (las páginas recargan al verlo subir).

export const fw = $state({ pendiente: null, version: 0 });

/** Tras una respuesta 202 de la API: muestra la barra de 60 s y avisa a las páginas. */
export function aplicado(r) {
  if (r?.pendiente) fw.pendiente = { ...r.pendiente, desde: Date.now() };
  fw.version++;
}

export function recargar() {
  fw.version++;
}

/** Texto de un error de la API: los de validación (422) vienen en `errores`. */
export function textoError(e) {
  const d = e?.datos;
  if (d?.errores?.length) return d.errores.join(' ');
  if (d?.error) return d.error;
  return e?.mensaje ?? String(e);
}

export const NOMBRE_ZONA_TIPO = {
  externa: 'Internet y lo que llega de fuera',
  interna: 'Tus equipos de confianza',
  gateway: 'Esta máquina (el propio servidor)',
  vpn: 'Dispositivos conectados por la VPN',
  dmz: 'Servidores expuestos que no deben ver tu red',
  personalizada: 'Zona creada por ti',
};

/** IPs propias y redes de la máquina a partir de `config.direcciones`. */
export function redesDe(config) {
  const propias = [];
  const conocidas = [];
  const lista = [];
  for (const [iface, ips] of Object.entries(config?.direcciones ?? {})) {
    for (const red of ips) {
      const [ip, n] = red.split('/');
      propias.push(ip);
      if (!ip.includes(':')) {
        const cidr = redBase(ip, Number(n));
        conocidas.push(cidr);
        if (Number(n) < 32) lista.push({ nombre: iface, cidr });
      }
    }
  }
  return { propias, conocidas, lista };
}

function redBase(ip, n) {
  const o = ip.split('.').map(Number);
  const v = ((o[0] << 24) >>> 0) + (o[1] << 16) + (o[2] << 8) + o[3];
  const m = n === 0 ? 0 : (~0 << (32 - n)) >>> 0;
  const b = (v & m) >>> 0;
  return `${b >>> 24}.${(b >>> 16) & 255}.${(b >>> 8) & 255}.${b & 255}/${n}`;
}

/** «80, 443» / «27015-27030» */
export function textoPuertos(lista) {
  return (lista ?? []).map((r) => (r.desde === r.hasta ? `${r.desde}` : `${r.desde}-${r.hasta}`)).join(', ');
}

export const PROTOCOLOS = [
  { valor: 'tcp_udp', texto: 'TCP y UDP' },
  { valor: 'tcp', texto: 'TCP' },
  { valor: 'udp', texto: 'UDP' },
];

export function textoProto(p) {
  return { tcp: 'TCP', udp: 'UDP', tcp_udp: 'TCP y UDP', icmp: 'ICMP (ping)', cualquiera: 'Cualquiera' }[p] ?? p;
}

/** Tiempo que falta, en llano: «3 h», «2 días». */
export function textoFalta(seg) {
  if (seg == null) return '';
  if (seg < 3600) return `${Math.max(1, Math.round(seg / 60))} min`;
  if (seg < 86400) return `${Math.round(seg / 3600)} h`;
  return `${Math.round(seg / 86400)} días`;
}
