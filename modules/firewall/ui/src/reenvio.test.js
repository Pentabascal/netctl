import { test } from 'node:test';
import assert from 'node:assert/strict';
import { aReenvio, deReenvio, formNuevo, puertoUnico } from './reenvio.js';

const nas = { tipo: 'equipo', mac: 'aa:bb:cc:dd:ee:01', ip: '192.168.1.20', nombre: 'NAS' };

test('puerto de fuera distinto → interno', () => {
  const f = { ...formNuevo(), equipo: nas, puertos: '22', fuera: '2222', plantilla: 'ssh' };
  const r = aReenvio(f, 'SSH');
  assert.deepEqual(r.externo, [{ desde: 2222, hasta: 2222 }]);
  assert.equal(r.interno, 22);
  assert.equal(r.nombre, 'NAS · SSH');
  assert.deepEqual(deReenvio({ ...r, id: 1 }).fuera, '2222');
  assert.deepEqual(deReenvio({ ...r, id: 1 }).puertos, '22');
});

test('rango y lista: sin puerto interno', () => {
  const r = aReenvio({ ...formNuevo(), equipo: nas, puertos: '27015-27030', fuera: '9999' });
  assert.deepEqual(r.externo, [{ desde: 27015, hasta: 27030 }]);
  assert.equal(r.interno, null);
  assert.equal(puertoUnico('80, 443'), null);
  assert.equal(puertoUnico('80'), 80);
});

test('mismo puerto fuera que dentro → sin interno', () => {
  const r = aReenvio({ ...formNuevo(), equipo: nas, puertos: '80', fuera: '80' });
  assert.equal(r.interno, null);
});

test('caducidad y desde', () => {
  const r = aReenvio({ ...formNuevo(), equipo: nas, puertos: '80', caduca: '1h', soloAlgunas: true, desde: [{ tipo: 'ip', red: '1.2.3.4/32' }] }, '', 1000);
  assert.equal(r.caduca, 4600);
  assert.equal(r.desde.length, 1);
  const f = deReenvio({ ...r, id: 1 });
  assert.equal(f.caduca, 'mantener');
  assert.equal(aReenvio(f, '', 2000).caduca, 4600);
  assert.deepEqual(aReenvio({ ...f, soloAlgunas: false }).desde, []);
});

test('equipo escrito a mano: nombre con la IP', () => {
  const r = aReenvio({ ...formNuevo(), equipo: { tipo: 'ip', red: '192.168.1.50/32' }, puertos: '25565' }, 'Minecraft');
  assert.equal(r.nombre, '192.168.1.50 · Minecraft');
});
