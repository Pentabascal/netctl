import { test } from 'node:test';
import assert from 'node:assert/strict';
import { deRegla, aRegla, opcionesOrigen, opcionesDestino, textoHorario, nuevaRegla, esDominio, bandera } from './politica.js';

const base = {
  id: 7, nombre: 'Cámaras', origen_zona: 'interna', destino_zona: 'externa', accion: 'bloquear',
  protocolo: 'tcp', numero_protocolo: 0,
  puertos: { tipo: 'lista', valor: [{ desde: 80, hasta: 80 }, { desde: 8000, hasta: 8100 }] },
  puertos_origen: { tipo: 'cualquiera' },
  origen: { tipo: 'cualquiera' }, destino: { tipo: 'cualquiera' },
  version_ip: 'ambas', estado: { tipo: 'todas' }, horario: { tipo: 'siempre' },
  log: false, activa: true, descripcion: '',
};

test('ida y vuelta', () => {
  const ejemplos = [
    { ...base, origen: { tipo: 'mac', mac: 'aa:bb:cc:dd:ee:ff' }, descripcion: 'Tele' },
    {
      ...base, protocolo: 'tcp_udp', puertos_origen: { tipo: 'grupo', valor: 3 }, version_ip: 'v4',
      origen: { tipo: 'equipo', mac: 'aa:bb:cc:00:00:01', ip: '192.168.1.5', nombre: 'Cámara' },
      destino: { tipo: 'interfaz', iface: 'eth0' },
      horario: { tipo: 'semanal', valor: { dias: [0, 4], franja: { desde: '08:00', hasta: '17:00' } } },
    },
    {
      ...base, protocolo: 'otro', numero_protocolo: 47, puertos: { tipo: 'cualquiera' }, accion: 'rechazar', log: true, activa: false,
      origen: { tipo: 'interfaz', iface: 'eth1' }, destino: { tipo: 'ip', red: '10.0.0.0/8' },
      estado: { tipo: 'elegir', valor: ['nueva', 'establecida'] },
      horario: { tipo: 'a_medida', valor: [{ dias: [5, 6], franja: { desde: '22:00', hasta: '06:00' } }] },
    },
    { ...base, estado: { tipo: 'solo_nuevas' }, horario: { tipo: 'una_vez', valor: { inicio: 100, fin: 200 } } },
    { ...base, horario: { tipo: 'diario', valor: { desde: '01:00', hasta: '02:00' } }, destino: { tipo: 'grupo', id: 4 } },
  ];
  for (const r of ejemplos) assert.deepEqual(aRegla(deRegla(r)), r);
});

test('regla nueva con lo recomendado', () => {
  const r = nuevaRegla('externa', 'gateway');
  assert.equal(r.accion, 'permitir');
  assert.equal(nuevaRegla('interna', 'externa').accion, 'bloquear');
  assert.equal(r.activa, true);
  assert.deepEqual(r.horario, { tipo: 'siempre' });
});

test('sin Red si la zona no tiene tarjetas', () => {
  assert.deepEqual(opcionesOrigen([]).map((o) => o.valor), ['cualquiera', 'equipo', 'ip', 'mac']);
  assert.deepEqual(opcionesOrigen(['eth1']).map((o) => o.valor), ['cualquiera', 'equipo', 'red', 'ip', 'mac']);
  assert.deepEqual(opcionesDestino(['eth0']).map((o) => o.valor), ['cualquiera', 'ip', 'red']);
  assert.deepEqual(opcionesDestino([]).map((o) => o.valor), ['cualquiera', 'ip']);
});

test('textoHorario', () => {
  assert.equal(textoHorario({ tipo: 'siempre' }), 'Siempre');
  assert.equal(textoHorario({ tipo: 'semanal', valor: { dias: [0, 4], franja: { desde: '08:00', hasta: '17:00' } } }), 'L, V 08:00–17:00');
  assert.equal(textoHorario({ tipo: 'diario', valor: { desde: '22:00', hasta: '06:00' } }), 'Cada día 22:00–06:00');
  assert.equal(textoHorario({ tipo: 'a_medida', valor: [{ dias: [5, 6], franja: { desde: '10:00', hasta: '12:00' } }, { dias: [2], franja: { desde: '18:00', hasta: '19:00' } }] }), 'S, D 10:00–12:00 · X 18:00–19:00');
});

test('ida y vuelta con dominio, país y app', () => {
  const ejemplos = [
    { ...base, destino: { tipo: 'dominio', nombres: ['tiktok.com', 'www.tiktok.com'] } },
    { ...base, origen_zona: 'externa', destino_zona: 'gateway', origen: { tipo: 'pais', codigos: ['CN', 'RU'] } },
    { ...base, destino: { tipo: 'app', apps: [126, 7], nombres: ['YouTube', 'HTTP'] } },
  ];
  for (const r of ejemplos) assert.deepEqual(aRegla(deRegla(r)), r);
  const f = deRegla(ejemplos[0]);
  f.destino.dominios = [' TikTok.com ', ''];
  assert.deepEqual(aRegla(f).destino, { tipo: 'dominio', nombres: ['tiktok.com'] });
  assert.ok(esDominio('a-b.example.org') && !esDominio('-a.com') && !esDominio('mal_dominio.com') && !esDominio(''));
  assert.equal(bandera('es'), '🇪🇸');
});

test('app desactivada con motivo', () => {
  const motivo = 'No está instalada la librería nDPI (paquete libndpi).';
  const app = opcionesDestino([], { externa: true, apps: { disponible: false, motivo } }).find((o) => o.valor === 'app');
  assert.equal(app.desactivado, true);
  assert.equal(app.ayuda, motivo);
  const ok = opcionesDestino([], { externa: true, apps: { disponible: true, apps: [] } }).find((o) => o.valor === 'app');
  assert.ok(!ok.desactivado);
});

test('solo hacia externa', () => {
  const v = (o) => o.map((x) => x.valor);
  assert.deepEqual(v(opcionesDestino(['eth1'])), ['cualquiera', 'ip', 'red']);
  assert.deepEqual(v(opcionesDestino([], { externa: true })), ['cualquiera', 'ip', 'dominio', 'pais', 'app']);
  assert.ok(!v(opcionesOrigen([])).includes('pais'));
  assert.ok(v(opcionesOrigen([], { externa: true })).includes('pais'));
});
