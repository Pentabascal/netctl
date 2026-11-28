import { test } from 'node:test';
import assert from 'node:assert/strict';
import { contadores, aplicarFiltros, filtrosVacios, deNat, aNat, nuevaNat } from './tabla.js';

const filas = [
  { tipo: 'firewall', id: 1, nombre: 'Web', accion: 'Permitir', protocolo: 'TCP', origen: 'Cualquiera', destino: 'Cualquiera', activa: true, de_serie: false },
  { tipo: 'firewall', id: 2, nombre: 'Cámaras', accion: 'Bloquear', protocolo: 'Cualquiera', origen: 'Tarjeta eth2', destino: 'Cualquiera', activa: false, de_serie: false },
  { tipo: 'nat', id: 3, nombre: 'Salida', accion: 'SNAT', protocolo: 'Cualquiera', origen: 'Cualquiera', destino: 'Cualquiera', activa: true, de_serie: false },
  { tipo: 'reenvio', id: 4, nombre: 'Minecraft', accion: 'Reenviar', protocolo: 'TCP', origen: 'Cualquiera', destino: 'NAS · 192.168.1.20', activa: false, de_serie: false },
  { tipo: 'firewall', id: 'serie_permitir', nombre: 'Permitir todo', accion: 'Permitir', protocolo: 'Cualquiera', origen: 'Cualquiera', destino: 'Cualquiera', activa: true, de_serie: true },
];

test('contadores_incluyen_pausadas', () => {
  const c = contadores(filas.filter((f) => !f.de_serie));
  assert.deepEqual(c.tipo, { firewall: 2, nat: 1, reenvio: 1 });
  assert.deepEqual(c.accion, { Permitir: 1, Bloquear: 1, SNAT: 1, Reenviar: 1 });
  assert.deepEqual(c.protocolo, { TCP: 2, Cualquiera: 2 });
  assert.equal(c.origen.Cualquiera, 3);
  assert.equal(c.destino['NAS · 192.168.1.20'], 1);
  const total = Object.values(c.tipo).reduce((a, b) => a + b, 0);
  assert.equal(total, 4, 'las pausadas también cuentan');
});

test('filtrar_por_tipo_y_accion', () => {
  const f = { ...filtrosVacios(), tipo: 'firewall' };
  assert.deepEqual(aplicarFiltros(filas, f).map((x) => x.id), [1, 2]);
  assert.deepEqual(aplicarFiltros(filas, { ...f, serie: true }).map((x) => x.id), [1, 2, 'serie_permitir']);
  assert.deepEqual(aplicarFiltros(filas, { ...f, accion: ['Bloquear'] }).map((x) => x.id), [2]);
  assert.deepEqual(aplicarFiltros(filas, { ...filtrosVacios(), accion: ['Permitir', 'Reenviar'] }).map((x) => x.id), [1, 4]);
  assert.deepEqual(aplicarFiltros(filas, { ...filtrosVacios(), texto: 'camaras' }).map((x) => x.id), [2]);
});

test('limpiar_filtros', () => {
  const v = filtrosVacios();
  assert.deepEqual(v, { tipo: 'todas', accion: [], protocolo: [], origen: [], destino: [], serie: false, texto: '' });
  assert.equal(aplicarFiltros(filas, v).length, 4);
  v.accion.push('x');
  assert.deepEqual(filtrosVacios().accion, [], 'cada vez una copia nueva');
});

test('nat ida y vuelta', () => {
  const n = {
    id: 7, nombre: 'Web', tipo: { tipo: 'dnat', ip: '192.168.1.10', puerto: 8080 }, iface: 'eth0',
    origen: { tipo: 'cualquiera' }, destino: { tipo: 'cualquiera' }, protocolo: 'tcp',
    puertos: { tipo: 'lista', valor: [{ desde: 80, hasta: 80 }] }, activa: true, descripcion: '',
  };
  const f = deNat(n);
  assert.equal(f.tipo, 'dnat');
  assert.equal(f.ip, '192.168.1.10');
  assert.equal(f.puertoNuevo, '8080');
  const { id, ...sinId } = n;
  assert.deepEqual(aNat(f), sinId);
  const m = aNat({ ...deNat(nuevaNat()), nombre: 'Todo fuera', iface: 'eth0' });
  assert.deepEqual(m.tipo, { tipo: 'masquerade' });
  assert.equal(m.iface, 'eth0');
  const s = aNat({ ...deNat(nuevaNat()), nombre: 's', tipo: 'snat', ip: ' 203.0.113.5 ', iface: '' });
  assert.deepEqual(s.tipo, { tipo: 'snat', ip: '203.0.113.5' });
  assert.equal(s.iface, null);
});
