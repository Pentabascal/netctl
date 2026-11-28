<script>
  // Panel lateral de una casilla de la matriz: política por defecto y reglas propias (en orden).
  // Al abrir una regla, el mismo panel pasa a su formulario.
  import { PanelLateral, Boton, Aviso, Interruptor, Icono, Chip, textoDireccion } from '@netctl/kit';
  import { aplicado, textoError, textoPuertos, textoProto } from './estado.svelte.js';
  import FormRegla from './FormRegla.svelte';

  let { api, config, origen, destino, oncerrar } = $props();

  const c = $derived(config.config);
  const zo = $derived(c.zonas.find((z) => z.id === origen));
  const zd = $derived(c.zonas.find((z) => z.id === destino));
  const celda = $derived(config.matriz.find((m) => m.origen === origen && m.destino === destino));
  const reglas = $derived(c.reglas.filter((r) => r.origen_zona === origen && r.destino_zona === destino));
  const nombresGrupos = $derived(Object.fromEntries([...c.grupos_ip, ...c.grupos_puertos].map((g) => [g.id, g.nombre])));

  let editando = $state(null); // regla, 'nueva' o null
  let ocupado = $state(false);
  let error = $state('');

  async function cambiar(politica) {
    if (politica === celda.politica) return;
    ocupado = true;
    error = '';
    try {
      aplicado(await api.put(`/politicas/${origen}/${destino}`, { politica, revision: c.revision }));
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }

  async function activar(r, activa) {
    error = '';
    try {
      aplicado(await api.put(`/reglas/${r.id}`, { regla: { ...r, activa }, revision: c.revision }));
    } catch (e) {
      error = textoError(e);
    }
  }

  async function mover(i, paso) {
    const ids = c.reglas.map((r) => r.id);
    const a = ids.indexOf(reglas[i].id);
    const b = ids.indexOf(reglas[i + paso].id);
    [ids[a], ids[b]] = [ids[b], ids[a]];
    error = '';
    try {
      aplicado(await api.post('/reglas/orden', { ids, revision: c.revision }));
    } catch (e) {
      error = textoError(e);
    }
  }

  function textoRegla(r) {
    const partes = [];
    if (r.protocolo !== 'cualquiera') partes.push(textoProto(r.protocolo));
    if (r.puertos?.tipo === 'lista') partes.push(`puertos ${textoPuertos(r.puertos.valor)}`);
    if (r.puertos?.tipo === 'grupo') partes.push(`puertos «${nombresGrupos[r.puertos.valor]}»`);
    if (r.origen?.tipo !== 'cualquiera') partes.push(`desde ${textoDireccion(r.origen, nombresGrupos)}`);
    if (r.destino?.tipo !== 'cualquiera') partes.push(`hacia ${textoDireccion(r.destino, nombresGrupos)}`);
    return partes.join(' · ') || 'Todo el tráfico';
  }

  const ACCION = { permitir: ['verde', 'Permitir'], bloquear: ['rojo', 'Bloquear'], rechazar: ['ambar', 'Rechazar'] };
</script>

<PanelLateral
  abierto
  titulo={editando ? (editando === 'nueva' ? 'Regla nueva' : editando.nombre) : `${zo?.nombre} → ${zd?.nombre}`}
  subtitulo={editando ? `${zo?.nombre} → ${zd?.nombre}` : 'Lo que pasa con el tráfico de una zona a otra'}
  icono="cortafuegos"
  ancho="520px"
  oncerrar={() => (editando ? (editando = null) : oncerrar())}
>
  {#if editando}
    <FormRegla {api} {config} {origen} {destino} regla={editando === 'nueva' ? null : editando} onhecho={() => (editando = null)} />
  {:else if celda}
    <section>
      <h3 class="nc-seccion">Si no coincide con ninguna regla</h3>
      <p class="frase">Lo que pasa con el tráfico de «{zo.nombre}» hacia «{zd.nombre}» que no coincide con las reglas de abajo.</p>
      <div class="politica" role="radiogroup" aria-label="Política por defecto">
        {#each ['permitir', 'bloquear'] as p (p)}
          <button type="button" role="radio" aria-checked={celda.politica === p} class="{p}" class:elegida={celda.politica === p} disabled={ocupado} onclick={() => cambiar(p)}>
            <Icono nombre={p === 'permitir' ? 'ok_circulo' : 'escudo'} tam={18} />
            <span>
              <b>{p === 'permitir' ? 'Permitir' : 'Bloquear'}{#if celda.de_serie === p}{" "}<small>(recomendado)</small>{/if}</b>
              <small>{p === 'permitir' ? 'Deja pasar todo salvo lo que bloqueen las reglas.' : 'Corta todo salvo lo que permitan las reglas.'}</small>
            </span>
          </button>
        {/each}
      </div>
      {#if origen === 'vpn' || zo?.tipo === 'vpn'}
        <Aviso tipo="info">
          Además, cada cliente VPN tiene sus propios permisos en
          <a href="/modulo/wireguard/clientes">WireGuard › Clientes</a>. Tiene que estar permitido en los dos sitios.
        </Aviso>
      {/if}
    </section>

    {#if zd?.tipo === 'gateway'}
      <section>
        <h3 class="nc-seccion">Siempre permitido</h3>
        <p class="frase">Estas reglas protegen tu acceso y no se pueden quitar.</p>
        <ul class="fijas">
          {#each config.fijas as f (f.id)}
            <li><Icono nombre="candado" tam={14} /><span><b>{f.nombre}</b><small>{f.descripcion}</small></span></li>
          {/each}
        </ul>
      </section>
    {/if}

    <section>
      <div class="cab">
        <h3 class="nc-seccion">Reglas</h3>
        <Boton tam="pequeno" icono="mas" variante="primario" onclick={() => (editando = 'nueva')}>Regla</Boton>
      </div>
      <p class="frase">Excepciones a lo de arriba. Se miran en orden: la primera que coincide decide.</p>
      {#if reglas.length}
        <ol class="reglas">
          {#each reglas as r, i (r.id)}
            {@const [color, txt] = ACCION[r.accion]}
            <li class:apagada={!r.activa}>
              <div class="orden">
                <button type="button" aria-label="Subir" disabled={i === 0} onclick={() => mover(i, -1)}><Icono nombre="flecha_arriba" tam={13} /></button>
                <button type="button" aria-label="Bajar" disabled={i === reglas.length - 1} onclick={() => mover(i, 1)}><Icono nombre="flecha_abajo" tam={13} /></button>
              </div>
              <button type="button" class="cuerpo" onclick={() => (editando = r)}>
                <span class="nombre"><Chip {color} punto={false}>{txt}</Chip> {r.nombre}</span>
                <small>{textoRegla(r)}</small>
              </button>
              <Interruptor activo={r.activa} etiqueta="Activa" onchange={(v) => activar(r, v)} />
            </li>
          {/each}
        </ol>
      {:else}
        <p class="vacio">Sin reglas propias: se aplica la política de arriba a todo.</p>
      {/if}
    </section>
    {#if error}<Aviso tipo="error">{error}</Aviso>{/if}
  {/if}
</PanelLateral>

<style>
  section {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 22px;
  }
  .frase {
    margin: -4px 0 0;
    color: var(--texto-2);
    font-size: 13px;
  }
  .cab {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .politica {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .politica button {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 12px;
    text-align: left;
    background: var(--superficie-2);
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    color: var(--texto-2);
    font: inherit;
    cursor: pointer;
  }
  .politica button span {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .politica b {
    color: var(--texto);
    font-weight: 600;
  }
  .politica small {
    font-size: 12px;
    color: var(--texto-3);
  }
  .politica b small {
    color: var(--verde);
    font-weight: 400;
  }
  .politica .permitir.elegida {
    border-color: var(--verde);
    background: var(--verde-tinte);
    color: var(--verde);
  }
  .politica .bloquear.elegida {
    border-color: var(--rojo);
    background: var(--rojo-tinte);
    color: var(--rojo);
  }
  .fijas,
  .reglas {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .fijas li {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 9px 12px;
    background: var(--superficie-2);
    border-radius: var(--radio-pq);
    color: var(--texto-2);
  }
  .fijas span {
    display: flex;
    flex-direction: column;
  }
  .fijas b {
    color: var(--texto);
    font-weight: 500;
  }
  small {
    color: var(--texto-3);
    font-size: 12px;
  }
  .reglas li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border: 1px solid var(--borde);
    border-radius: var(--radio-pq);
    background: var(--superficie);
  }
  .reglas li.apagada .cuerpo {
    opacity: 0.55;
  }
  .orden {
    display: flex;
    flex-direction: column;
  }
  .orden button {
    border: 0;
    background: none;
    color: var(--texto-3);
    cursor: pointer;
    padding: 1px;
    display: inline-flex;
  }
  .orden button:hover:not(:disabled) {
    color: var(--texto);
  }
  .orden button:disabled {
    opacity: 0.3;
    cursor: default;
  }
  .cuerpo {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    border: 0;
    background: none;
    color: var(--texto);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .nombre {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 500;
  }
  .vacio {
    color: var(--texto-3);
    font-size: 13px;
  }
</style>
