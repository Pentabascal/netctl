// Texto y color del estado systemd de un módulo.
export function estadoModulo(activo) {
  switch (activo) {
    case 'active':
      return { texto: 'En marcha', color: 'verde' };
    case 'activating':
    case 'reloading':
      return { texto: 'Arrancando', color: 'azul' };
    case 'deactivating':
      return { texto: 'Parando', color: 'ambar' };
    case 'failed':
      return { texto: 'Con fallos', color: 'rojo' };
    case 'inactive':
      return { texto: 'Parado', color: 'gris' };
    default:
      return { texto: 'Desconocido', color: 'gris' };
  }
}
