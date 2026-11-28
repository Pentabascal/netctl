<script>
  // Panel lateral de un reenvío: todo editable y explicado, más duplicar y borrar.
  import { untrack } from 'svelte';
  import { PanelLateral, Campo, CampoNumero, SelectorEquipo, Opcion, Boton, Aviso, Icono } from '@netctl/kit';
  import { aplicado, textoError, redesDe, PROTOCOLOS, textoFalta } from './estado.svelte.js';
  import { deReenvio, aReenvio, puertoUnico, CADUCA } from './reenvio.js';

  let { api, config, reenvio, oncerrar } = $props();

  const c = $derived(config.config);
  const redes = $derived(redesDe(c));
  const externas = $derived(c.zonas.filter((z) => z.tipo === 'externa').flatMap((z) => Object.entries(c.interfaces).filter(([, x]) => x === z.id).map(([i]) => i)));

  let f = $state(untrack(() => deReenvio(reenvio)));
  let puertosValidos = $state(true);
  let fueraValido = $state(true);
  let comprobado = $state(null);
  let ocupado = $state(false);
  let error = $state('');

  const unico = $derived(puertoUnico(f.puertos));
  const datos = $derived(aReenvio(f));

  let t;
  $effect(() => {
    const d = JSON.stringify({ ...datos, id: reenvio.id });
    clearTimeout(t);
    t = setTimeout(async () => {
      try {
        comprobado = await api.post('/reenvios/comprobar', { reenvio: JSON.parse(d) });
      } catch {
        comprobado = null;
      }
    }, 250);
  });

  async function hacer(accion) {
    ocupado = true;
    error = '';
    try {
      if (accion === 'guardar') aplicado(await api.put(`/reenvios/${reenvio.id}`, { reenvio: datos, revision: c.revision }));
      else if (accion === 'borrar') aplicado(await api.del(`/reenvios/${reenvio.id}?revision=${c.revision}`));
      else aplicado(await api.post(`/reenvios/${reenvio.id}/duplicar`, { revision: c.revision }));
      oncerrar();
    } catch (e) {
      error = textoError(e);
    } finally {
      ocupado = false;
    }
  }

  const caducaOpciones = $derived(
    f.caducaActual ? [{ valor: 'mantener', texto: `Como está (${reenvio.caduca_en ? 'en ' + textoFalta(reenvio.caduca_en) : 'ya caducó'})` }, ...CADUCA] : CADUCA,
  );
  const errores = $derived(comprobado?.errores ?? []);
</script>

<PanelLateral abierto titulo={reenvio.nombre} subtitulo="Reenvío de puerto" icono="globo" ancho="560px" {oncerrar}>
  <div class="form">
    {#if reenvio.como_conectarse?.length}
      <div class="como">
        <Icono nombre="enlace" tam={15} />
        <span>Desde fuera: {#each reenvio.como_conectarse as d, i (d)}{#if i}, {/if}<code>{d}</code>{/each}</span>
      </div>
    {/if}

    <Campo etiqueta="Nombre" bind:valor={f.nombre} ayuda="Para reconocerlo en la lista." />
    <SelectorEquipo
      etiqueta="Equipo"
      bind:valor={f.equipo}
      soloIpv4
      propias={redes.propias}
      conocidas={redes.conocidas}
      ayuda="Si lo eliges de la lista, el reenvío le sigue aunque cambie de IP."
    />
    <div class="dos">
      <CampoNumero
        etiqueta="Puerto del equipo"
        bind:valor={f.puertos}
        bind:valido={puertosValidos}
        puertos
        requerido
        ayuda="Puerto, lista o rango en el que escucha el servicio."
        mas={{ ejemplo: '25565  ·  80, 443  ·  27015-27030' }}
      />
      <Campo etiqueta="Protocolo" tipo="select" bind:valor={f.protocolo} opciones={PROTOCOLOS} ayuda="Si no lo sabes, deja TCP y UDP." />
    </div>
    {#if unico != null}
      <CampoNumero
        etiqueta="Puerto de fuera (opcional)"
        bind:valor={f.fuera}
        bind:valido={fueraValido}
        placeholder="El mismo: {unico}"
        ayuda="El que se usa desde internet. Vacío = el mismo que el del equipo."
      />
    {/if}

    <div class="campo">
      <span class="etq">Desde dónde</span>
      <div class="opciones" role="radiogroup" aria-label="Desde dónde">
        <button type="button" role="radio" aria-checked={!f.soloAlgunas} class:elegida={!f.soloAlgunas} onclick={() => (f.soloAlgunas = false)}>Desde cualquier sitio de internet</button>
        <button type="button" role="radio" aria-checked={f.soloAlgunas} class:elegida={f.soloAlgunas} onclick={() => (f.soloAlgunas = true)}>Solo desde estas IPs</button>
      </div>
    </div>
    {#if f.soloAlgunas}
      <SelectorEquipo bind:valor={f.desde} varios redes soloIpv4 equipos={[]} grupos={c.grupos_ip.map((g) => ({ id: g.id, nombre: g.nombre }))} placeholder="Escribe una IP, red o rango de internet" />
    {/if}
    <Campo etiqueta="Caduca" tipo="select" bind:valor={f.caduca} opciones={caducaOpciones} ayuda="Al caducar se desactiva solo, sin borrarse." />

    <h3 class="nc-seccion">Menos usado</h3>
    {#if externas.length > 1}
      <Campo
        etiqueta="Conexión a internet (WAN) por la que entra"
        tipo="select"
        bind:valor={f.wan}
        opciones={[{ valor: '', texto: 'Todas (recomendado)' }, ...externas.map((i) => ({ valor: i, texto: i }))]}
        ayuda="Con varias conexiones a internet, por cuál se abre el puerto."
      />
    {/if}
    <Opcion
      titulo="Acceso desde dentro con la IP pública (recomendado)"
      texto="Los equipos de tu red también llegan usando la dirección de internet, igual que desde fuera (NAT loopback, «hairpin»)."
      bind:activo={f.loopback}
    />
    <Opcion
      titulo="Registrar conexiones (log)"
      texto="Anota en el registro del sistema cada conexión nueva. Solo para investigar."
      bind:activo={f.log}
    />
    <Opcion titulo="Activo" texto="Apágalo para cerrar el puerto sin borrar el reenvío." bind:activo={f.activo} />

    {#each errores as e (e)}<Aviso tipo="error">{e}</Aviso>{/each}
    {#each comprobado?.avisos ?? [] as a (a)}<Aviso tipo="aviso">{a}</Aviso>{/each}
    {#if error}<Aviso tipo="error">{error}</Aviso>{/if}

    <div class="pie">
      <Boton variante="peligro" icono="papelera" desactivado={ocupado} onclick={() => hacer('borrar')}>Borrar</Boton>
      <Boton icono="copiar" desactivado={ocupado} onclick={() => hacer('duplicar')} titulo="El mismo servicio a otro equipo (sale desactivado)">Duplicar</Boton>
      <span class="hueco"></span>
      <Boton onclick={oncerrar}>Cancelar</Boton>
      <Boton variante="primario" cargando={ocupado} desactivado={!puertosValidos || !fueraValido || !f.equipo || errores.length > 0} onclick={() => hacer('guardar')}>Guardar</Boton>
    </div>
  </div>
</PanelLateral>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .como {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    background: var(--azul-tinte);
    border-radius: var(--radio-pq);
    color: var(--texto-2);
    font-size: 13px;
  }
  code {
    font-family: var(--mono);
    color: var(--texto);
  }
  .dos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .campo {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .etq {
    font-size: 12.5px;
    font-weight: 500;
    color: var(--texto-2);
  }
  .opciones {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }
  .opciones button {
    padding: 9px 10px;
    background: var(--superficie-2);
    border: 1px solid var(--borde);
    border-radius: var(--radio-pq);
    color: var(--texto);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }
  .opciones button.elegida {
    border-color: var(--azul);
    background: var(--azul-tinte);
  }
  .pie {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    padding-top: 8px;
    border-top: 1px solid var(--borde-suave);
  }
  .hueco {
    flex: 1;
  }
</style>
