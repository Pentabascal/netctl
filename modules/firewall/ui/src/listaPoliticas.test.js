import { test } from 'node:test';
import assert from 'node:assert/strict';
import { filtrar, COLUMNAS, mover, ORDEN_ZONAS, ordenarZonas } from './listaPoliticas.js';

const r = (id, o, d, extra = {}) => ({
  id, nombre: `regla ${id}`, origen_zona: o, destino_zona: d, version_ip: 'ambas', activa: true,
  zona_origen_nombre: o, zona_destino_nombre: d, texto_origen: 'Cualquiera', texto_destino: 'Cualquiera',
  texto_puertos: '80', texto_puertos_origen: 'Cualquiera', ...extra,
});
const todas = { par: null, v4: true, v6: true, texto: '' };

test('filtra por pareja', () => {
  const l = [r(1, 'interna', 'externa'), r(2, 'externa', 'interna'), r(3, 'interna', 'externa')];
  assert.deepEqual(filtrar(l, { ...todas, par: ['interna', 'externa'] }).map((x) => x.id), [1, 3]);
  assert.equal(filtrar(l, todas).length, 3);
});

test('quita v6', () => {
  const l = [r(1, 'a', 'b', { version_ip: 'v6' }), r(2, 'a', 'b', { version_ip: 'ambas' }), r(3, 'a', 'b', { version_ip: 'v4' })];
  assert.deepEqual(filtrar(l, { ...todas, v6: false }).map((x) => x.id), [2, 3]);
  assert.deepEqual(filtrar(l, { ...todas, v4: false }).map((x) => x.id), [1, 2]);
  assert.deepEqual(filtrar(l, { ...todas, v4: false, v6: false }), []);
});

test('busca sin acentos ni mayúsculas', () => {
  const l = [r(1, 'interna', 'externa', { nombre: 'Cámaras sin internet' }), r(2, 'a', 'b', { texto_destino: 'Impresora · 192.168.1.40' })];
  assert.deepEqual(filtrar(l, { ...todas, texto: 'camaras' }).map((x) => x.id), [1]);
  assert.deepEqual(filtrar(l, { ...todas, texto: 'IMPRESORA' }).map((x) => x.id), [2]);
  assert.deepEqual(filtrar(l, { ...todas, texto: '192.168.1.40' }).map((x) => x.id), [2]);
});

test('mover respeta los límites', () => {
  assert.deepEqual(mover([1, 2, 3], 2, -1), [2, 1, 3]);
  assert.deepEqual(mover([1, 2, 3], 2, 1), [1, 3, 2]);
  assert.deepEqual(mover([1, 2, 3], 1, -1), [1, 2, 3]);
  assert.deepEqual(mover([1, 2, 3], 3, 1), [1, 2, 3]);
  assert.deepEqual(mover([1, 2, 3], 9, 1), [1, 2, 3]);
});

test('columnas del spec', () => {
  assert.deepEqual(COLUMNAS.map((c) => c.id), [
    'nombre', 'accion', 'version_ip', 'protocolo', 'zona_origen', 'origen', 'puerto_origen', 'zona_destino', 'destino', 'puerto_destino',
  ]);
});

test('zonas en el orden de UniFi', () => {
  const z = ['personalizada', 'dmz', 'gateway', 'externa', 'hotspot', 'interna', 'vpn'].map((tipo, i) => ({ id: `z${i}`, tipo }));
  assert.deepEqual(ordenarZonas(z).map((x) => x.tipo), ORDEN_ZONAS.concat('personalizada'));
});
