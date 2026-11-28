// Pruebas de la lógica del kit: node --test web/kit/src/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  leerEntrada,
  leerPuertos,
  numeroEnRango,
  soloDigitos,
  ipEnRed,
  textoDireccion,
  buscarEquipos,
  agruparEquipos,
} from './validar.js';

test('IP sola → red /32 y /128', () => {
  assert.deepEqual(leerEntrada('192.168.1.50').dir, { tipo: 'ip', red: '192.168.1.50/32' });
  assert.deepEqual(leerEntrada(' fd00::5 ').dir, { tipo: 'ip', red: 'fd00::5/128' });
  assert.equal(leerEntrada('192.168.1.300').ok, false);
  assert.equal(leerEntrada('hola').ok, false);
  assert.equal(leerEntrada('').ok, false);
});

test('redes y rangos solo si el campo los admite', () => {
  assert.equal(leerEntrada('192.168.1.0/24').ok, false);
  assert.match(leerEntrada('192.168.1.0/24').error, /solo una IP/);
  assert.deepEqual(leerEntrada('192.168.1.0/24', { redes: true }).dir, { tipo: 'ip', red: '192.168.1.0/24' });
  assert.equal(leerEntrada('192.168.1.0/33', { redes: true }).ok, false);
  assert.deepEqual(leerEntrada('192.168.1.10-192.168.1.20', { redes: true }).dir, {
    tipo: 'rango',
    desde: '192.168.1.10',
    hasta: '192.168.1.20',
  });
  assert.match(leerEntrada('192.168.1.20-192.168.1.10', { redes: true }).error, /al revés/);
  assert.match(leerEntrada('192.168.1.1-fd00::1', { redes: true }).error, /IPv4 e IPv6/);
});

test('no vale la IP de la propia máquina; aviso si no está en una red conocida', () => {
  const op = { propias: ['192.168.1.1'], conocidas: ['192.168.1.0/24', '10.8.0.0/24'] };
  assert.match(leerEntrada('192.168.1.1', op).error, /esta máquina/);
  const r = leerEntrada('172.30.0.5', op);
  assert.equal(r.ok, true);
  assert.match(r.aviso, /ninguna red/);
  assert.equal(leerEntrada('10.8.0.9', op).aviso, undefined);
});

test('ipEnRed', () => {
  assert.equal(ipEnRed('192.168.1.77', '192.168.1.0/24'), true);
  assert.equal(ipEnRed('192.168.2.77', '192.168.1.0/24'), false);
  assert.equal(ipEnRed('10.0.0.1', '0.0.0.0/0'), true);
});

test('puertos como texto', () => {
  assert.deepEqual(leerPuertos('80, 443,8000-8100').valor, [
    { desde: 80, hasta: 80 },
    { desde: 443, hasta: 443 },
    { desde: 8000, hasta: 8100 },
  ]);
  assert.match(leerPuertos('90-80').error, /al revés/);
  assert.equal(leerPuertos('0').ok, false);
  assert.equal(leerPuertos('70000').ok, false);
  assert.equal(leerPuertos('').ok, false);
  assert.equal(leerPuertos('8o').ok, false);
});

test('números con comprobador', () => {
  assert.equal(soloDigitos('8a0-8 '), '808');
  assert.deepEqual(numeroEnRango('25', 1, 65535), { ok: true, valor: 25 });
  assert.match(numeroEnRango('0', 1, 65535).error, /entre 1 y 65535/);
  assert.equal(numeroEnRango('', 1, 10).ok, false);
});

test('texto de una dirección', () => {
  assert.equal(textoDireccion({ tipo: 'cualquiera' }), 'Cualquiera');
  assert.equal(textoDireccion({ tipo: 'equipo', mac: 'aa', ip: '192.168.1.20', nombre: 'NAS' }), 'NAS · 192.168.1.20');
  assert.equal(textoDireccion({ tipo: 'ip', red: '192.168.1.50/32' }), '192.168.1.50');
  assert.equal(textoDireccion({ tipo: 'ip', red: '192.168.1.0/24' }), '192.168.1.0/24');
  assert.equal(textoDireccion({ tipo: 'rango', desde: 'a', hasta: 'b' }), 'a – b');
  assert.equal(textoDireccion({ tipo: 'grupo', id: 3 }, { 3: 'Oficina' }), 'Grupo «Oficina»');
});

const EQUIPOS = [
  { nombre: '', ip: '192.168.1.30', mac: 'aa:bb:cc:00:00:30', red: 'eth1', conectado: false, origen: 'red' },
  { nombre: 'NAS', ip: '192.168.1.20', mac: 'aa:bb:cc:00:00:20', red: 'eth1', conectado: true, origen: 'red', fabricante: 'Synology' },
  { nombre: 'Móvil de Ana', ip: '10.8.0.2', mac: '', red: 'VPN', conectado: true, origen: 'vpn' },
];

test('buscar equipos por nombre, IP, MAC o fabricante (sin acentos)', () => {
  assert.equal(buscarEquipos(EQUIPOS, 'nas').length, 1);
  assert.equal(buscarEquipos(EQUIPOS, 'movil').length, 1);
  assert.equal(buscarEquipos(EQUIPOS, '192.168.1.3').length, 1);
  assert.equal(buscarEquipos(EQUIPOS, '00:20').length, 1);
  assert.equal(buscarEquipos(EQUIPOS, 'synol').length, 1);
  assert.equal(buscarEquipos(EQUIPOS, '').length, 3);
});

test('agrupar por red, conectados primero', () => {
  const g = agruparEquipos(EQUIPOS);
  assert.deepEqual(g.map((x) => x.red), ['eth1', 'VPN']);
  assert.equal(g[0].equipos[0].nombre, 'NAS');
});
