import { test } from 'node:test';
import assert from 'node:assert/strict';
import { analizar } from './rutas.js';

test('rutas de siempre', () => {
  assert.deepEqual(analizar('/'), { pagina: 'inicio' });
  assert.deepEqual(analizar('/modulos'), { pagina: 'modulos' });
  assert.deepEqual(analizar('/modulo/x/y/z'), { pagina: 'modulo', id: 'x', sub: 'y/z' });
  assert.deepEqual(analizar('/nada'), { pagina: 'no-existe' });
});

test('ajustes del núcleo', () => {
  assert.deepEqual(analizar('/ajustes/password'), { pagina: 'ajustes', seccion: 'password' });
  assert.deepEqual(analizar('/ajustes'), { pagina: 'ajustes', seccion: undefined });
});

test('ajustes de un módulo', () => {
  assert.deepEqual(analizar('/ajustes/m/firewall/zonas'), { pagina: 'ajustes', seccion: 'm', id: 'firewall', sub: 'zonas' });
  assert.deepEqual(analizar('/ajustes/m/firewall'), { pagina: 'ajustes', seccion: 'm', id: 'firewall', sub: '' });
});
