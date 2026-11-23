import { formatoBytes } from '@netctl/kit';

/** Estado legible de un cliente: conectado / desconectado / nunca / desactivado.
 *  Con la interfaz caída no se sabe si alguno ha conectado alguna vez: todos «Desconectado». */
export function estadoCliente(c, caida = false) {
  if (!c.activo) return { clave: 'desactivado', texto: 'Desactivado', color: 'gris', orden: 3 };
  if (caida) return { clave: 'desconectado', texto: 'Desconectado', color: 'gris', orden: 1 };
  if (c.estado?.conectado) return { clave: 'conectado', texto: 'Conectado', color: 'verde', orden: 0 };
  if (!c.estado?.ultimo_handshake) return { clave: 'nunca', texto: 'Nunca conectado', color: 'gris', orden: 2 };
  return { clave: 'desconectado', texto: 'Desconectado', color: 'gris', orden: 1 };
}

/** Bytes por segundo: «1,2 MB/s»; 0 → «—». */
export function formatoTasa(n) {
  if (!n) return '0 B/s';
  return `${formatoBytes(n)}/s`;
}

/** Mezcla una foto de `/vivo` (SSE) en la lista de clientes. */
export function mezclarVivo(datos, vivo) {
  if (!datos || !vivo || vivo.interfaz !== datos.interfaz) return datos;
  const porClave = new Map((vivo.peers ?? []).map((p) => [p.clave_publica, p]));
  const clientes = datos.clientes.map((c) => ({
    ...c,
    // Igual que el backend: un cliente que no sale en `wg show` cuenta como «nunca» y sin tráfico.
    estado: (c.activo && porClave.get(c.clave_publica)) || {
      clave_publica: c.clave_publica,
      endpoint: null,
      ultimo_handshake: null,
      rx: 0,
      tx: 0,
      conectado: false,
      tasa_rx: 0,
      tasa_tx: 0,
    },
  }));
  return {
    ...datos,
    clientes,
    interfaz_activa: vivo.activa,
    aviso: vivo.aviso,
    historial: vivo.historial ?? [],
    metricas: calcularMetricas(clientes),
  };
}

export function calcularMetricas(clientes) {
  const m = { conectados: 0, total: clientes.length, activos: 0, rx: 0, tx: 0, tasa_rx: 0, tasa_tx: 0 };
  for (const c of clientes) {
    if (c.activo) m.activos++;
    if (c.estado?.conectado) m.conectados++;
    m.rx += c.estado?.rx ?? 0;
    m.tx += c.estado?.tx ?? 0;
    m.tasa_rx += c.estado?.tasa_rx ?? 0;
    m.tasa_tx += c.estado?.tasa_tx ?? 0;
  }
  return m;
}

/** ¿El cambio de ajustes del servidor reinicia la interfaz? (mismos campos que el backend) */
export const CAMPOS_REINICIO = ['interfaz', 'puerto', 'red_vpn', 'ip_servidor', 'mtu'];

export async function copiar(texto) {
  try {
    await navigator.clipboard.writeText(texto);
    return true;
  } catch {
    return false;
  }
}
