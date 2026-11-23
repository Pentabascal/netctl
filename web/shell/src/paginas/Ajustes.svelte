<script>
  import { onMount } from 'svelte';
  import { apiNucleo, Barra, Pestanas, Tarjeta, Campo, Boton, Aviso, Chip, Qr, Icono, Confirmacion60 } from '@netctl/kit';
  import { sesion, cerrarLocal } from '../sesion.svelte.js';
  import { ruta, analizar, navegar } from '../ruta.svelte.js';

  const secciones = ['password', 'totp', 'acceso'];
  let seccion = $state(secciones.includes(analizar(ruta.actual).seccion) ? analizar(ruta.actual).seccion : 'password');

  function cambiarSeccion(s) {
    navegar(`/ajustes/${s}`, true);
  }

  // ------------------------------------------------------------ contraseña
  let pwActual = $state('');
  let pwNueva = $state('');
  let pwRepetir = $state('');
  let pwError = $state('');
  let pwGuardando = $state(false);
  const errNueva = $derived(pwNueva && pwNueva.length < 10 ? 'Debe tener al menos 10 caracteres' : '');
  const errRepetir = $derived(pwRepetir && pwRepetir !== pwNueva ? 'No coincide con la nueva' : '');

  async function cambiarPassword(e) {
    e.preventDefault();
    if (errNueva || errRepetir || !pwActual || !pwNueva) return;
    pwGuardando = true;
    pwError = '';
    try {
      await apiNucleo.put('/api/ajustes/password', { actual: pwActual, nueva: pwNueva });
      cerrarLocal('Contraseña cambiada. Se han cerrado todas las sesiones; entra con la nueva.');
    } catch (err) {
      pwError = err.mensaje;
    } finally {
      pwGuardando = false;
    }
  }

  // ------------------------------------------------------------ doble factor
  let totpAlta = $state(null); // { secreto, uri }
  let totpCodigo = $state('');
  let totpError = $state('');
  let totpCargando = $state(false);
  let totpHecho = $state(false);
  let copiado = $state(false);

  async function empezarTotp() {
    totpCargando = true;
    totpError = '';
    try {
      totpAlta = await apiNucleo.post('/api/ajustes/totp');
    } catch (err) {
      totpError = err.mensaje;
    } finally {
      totpCargando = false;
    }
  }

  async function confirmarTotp(e) {
    e.preventDefault();
    totpCargando = true;
    totpError = '';
    try {
      await apiNucleo.post('/api/ajustes/totp/confirmar', { codigo: totpCodigo });
      sesion.yo.totp_activo = true;
      totpAlta = null;
      totpHecho = true;
      totpCodigo = '';
    } catch (err) {
      totpError = err.mensaje;
    } finally {
      totpCargando = false;
    }
  }

  async function copiarSecreto() {
    try {
      await navigator.clipboard.writeText(totpAlta.secreto);
      copiado = true;
      setTimeout(() => (copiado = false), 1500);
    } catch {
      /* sin portapapeles */
    }
  }

  const secretoBonito = $derived(totpAlta?.secreto?.match(/.{1,4}/g)?.join(' ') ?? '');

  // ------------------------------------------------------------ acceso
  let acceso = $state(null); // { alcance, totp_activo, pendiente }
  let eleccion = $state('solo_lan');
  let accesoError = $state('');
  let accesoAviso = $state('');
  let aplicando = $state(false);
  let pendiente = $state(null); // { id, segundos }

  async function cargarAcceso() {
    try {
      acceso = await apiNucleo.get('/api/ajustes/acceso');
      eleccion = acceso.alcance;
      if (acceso.pendiente != null && !pendiente) pendiente = { id: acceso.pendiente, segundos: null };
      if (sesion.yo) sesion.yo.alcance = acceso.alcance;
    } catch (err) {
      accesoError = err.mensaje;
    }
  }

  async function aplicarAcceso() {
    aplicando = true;
    accesoError = '';
    accesoAviso = '';
    try {
      const r = await apiNucleo.put('/api/ajustes/acceso', { alcance: eleccion });
      if (r.id != null) pendiente = { id: r.id, segundos: r.segundos ?? 60 };
      else await cargarAcceso();
    } catch (err) {
      accesoError = err.mensaje;
    } finally {
      aplicando = false;
    }
  }

  async function confirmarAcceso() {
    const r = await apiNucleo.post('/api/ajustes/acceso/confirmar', { id: pendiente.id });
    pendiente = null;
    acceso = r;
    eleccion = r.alcance;
    sesion.yo.alcance = r.alcance;
    accesoAviso = r.alcance === 'internet' ? 'Listo: el panel ya acepta conexiones desde Internet.' : 'Listo: el panel solo acepta conexiones de la LAN y la VPN.';
  }

  function expirado() {
    pendiente = null;
    accesoError = 'No se confirmó a tiempo y el cambio se ha deshecho.';
    setTimeout(cargarAcceso, 1500);
  }

  onMount(cargarAcceso);

  const opciones = [
    {
      id: 'solo_lan',
      titulo: 'Solo LAN y VPN',
      texto: 'Solo se puede entrar desde redes privadas (192.168.x.x, 10.x.x.x…) y desde la VPN WireGuard.',
      icono: 'casa_red',
      recomendado: true,
    },
    {
      id: 'internet',
      titulo: 'Internet',
      texto: 'Se puede entrar desde cualquier red. Exige doble factor y deberías usar una contraseña larga.',
      icono: 'globo',
    },
  ];
</script>

<div class="nc-pagina">
  <Barra titulo="Ajustes" subtitulo="Seguridad y acceso al panel" />

  <Pestanas
    bind:activa={seccion}
    onchange={cambiarSeccion}
    pestanas={[
      { id: 'password', titulo: 'Contraseña' },
      { id: 'totp', titulo: 'Doble factor' },
      { id: 'acceso', titulo: 'Acceso' },
    ]}
  />

  <div class="contenido">
    {#if seccion === 'password'}
      <Tarjeta titulo="Contraseña del administrador" subtitulo="Usuario «admin»" icono="llave">
        <form class="formulario" onsubmit={cambiarPassword}>
          {#if pwError}<Aviso tipo="error">{pwError}</Aviso>{/if}
          <Campo etiqueta="Contraseña actual" tipo="password" bind:valor={pwActual} autocompletar="current-password" requerido />
          <div class="dos">
            <Campo
              etiqueta="Contraseña nueva"
              tipo="password"
              bind:valor={pwNueva}
              autocompletar="new-password"
              ayuda="Mínimo 10 caracteres. Una frase larga es más segura y fácil de recordar."
              error={errNueva}
              requerido
            />
            <Campo etiqueta="Repite la nueva" tipo="password" bind:valor={pwRepetir} autocompletar="new-password" error={errRepetir} requerido />
          </div>
          <div class="pie">
            <span class="nota"><Icono nombre="info" tam={14} />Al cambiarla se cierran todas las sesiones, también esta.</span>
            <Boton
              variante="primario"
              tipo="submit"
              cargando={pwGuardando}
              desactivado={!pwActual || !pwNueva || !pwRepetir || !!errNueva || !!errRepetir}>Cambiar contraseña</Boton
            >
          </div>
        </form>
      </Tarjeta>
    {:else if seccion === 'totp'}
      <Tarjeta titulo="Doble factor (TOTP)" subtitulo="Un código de 6 cifras del móvil además de la contraseña" icono="movil">
        {#snippet acciones()}
          <Chip color={sesion.yo?.totp_activo ? 'verde' : 'gris'}>{sesion.yo?.totp_activo ? 'Activado' : 'Desactivado'}</Chip>
        {/snippet}
        {#if totpError}<div class="espacio"><Aviso tipo="error">{totpError}</Aviso></div>{/if}
        {#if sesion.yo?.totp_activo}
          <div class="formulario">
            {#if totpHecho}<Aviso tipo="ok">Doble factor activado. A partir de ahora se pedirá el código al entrar.</Aviso>{/if}
            <p class="explica">
              Al entrar se pide el código que muestra tu app de autenticación. Si pierdes el móvil, quítalo desde el
              servidor con <code>netctl-core admin quitar-totp</code> (el panel vuelve a «Solo LAN y VPN»).
            </p>
          </div>
        {:else if !totpAlta}
          <div class="formulario">
            <p class="explica">
              Protege el panel aunque alguien averigüe la contraseña. Necesitas una app como Google Authenticator,
              Aegis, 2FAS o la de tu gestor de contraseñas. Es obligatorio para abrir el panel a Internet.
            </p>
            <div class="pie fin"><Boton variante="primario" icono="candado" cargando={totpCargando} onclick={empezarTotp}>Activar doble factor</Boton></div>
          </div>
        {:else}
          <form class="alta" onsubmit={confirmarTotp}>
            <Qr texto={totpAlta.uri} tam={196} />
            <div class="pasos">
              <div class="paso">
                <span class="num">1</span>
                <div>
                  <strong>Escanea el código con la app</strong>
                  <p>¿No puedes escanear? Escribe esta clave a mano:</p>
                  <div class="secreto">
                    <code>{secretoBonito}</code>
                    <Boton variante="fantasma" tam="pequeno" icono={copiado ? 'ok' : 'copiar'} titulo="Copiar clave" onclick={copiarSecreto} />
                  </div>
                </div>
              </div>
              <div class="paso">
                <span class="num">2</span>
                <div class="crece">
                  <strong>Escribe el código que muestra</strong>
                  <div class="codigo">
                    <Campo bind:valor={totpCodigo} inputmode="numeric" maxlength={6} placeholder="123456" mono enfocar autocompletar="one-time-code" />
                    <Boton variante="primario" tipo="submit" cargando={totpCargando} desactivado={totpCodigo.trim().length < 6}>Activar</Boton>
                  </div>
                </div>
              </div>
              <button type="button" class="cancelar" onclick={() => ((totpAlta = null), (totpError = ''))}>Cancelar</button>
            </div>
          </form>
        {/if}
      </Tarjeta>
    {:else}
      <Tarjeta titulo="Desde dónde se puede entrar" subtitulo="El cambio se deshace solo si no lo confirmas en 60 s" icono="globo">
        <div class="formulario">
          {#if accesoError}<Aviso tipo="error">{accesoError}</Aviso>{/if}
          {#if accesoAviso}<Aviso tipo="ok" cerrable oncerrar={() => (accesoAviso = '')}>{accesoAviso}</Aviso>{/if}
          <div class="opciones" role="radiogroup" aria-label="Acceso al panel">
            {#each opciones as o (o.id)}
              {@const bloqueada = o.id === 'internet' && !sesion.yo?.totp_activo}
              <button
                type="button"
                role="radio"
                aria-checked={eleccion === o.id}
                class="opcion"
                class:elegida={eleccion === o.id}
                disabled={bloqueada || !!pendiente}
                onclick={() => (eleccion = o.id)}
              >
                <span class="radio"></span>
                <span class="o-icono"><Icono nombre={o.icono} tam={20} /></span>
                <span class="o-texto">
                  <span class="o-titulo">
                    {o.titulo}
                    {#if o.recomendado}<Chip color="azul" punto={false}>Recomendado</Chip>{/if}
                    {#if acceso?.alcance === o.id}<Chip color="verde">Actual</Chip>{/if}
                  </span>
                  <span class="o-desc">{o.texto}</span>
                </span>
              </button>
            {/each}
          </div>
          {#if !sesion.yo?.totp_activo}
            <Aviso tipo="info">
              Para abrir el panel a Internet activa antes el doble factor.
              {#snippet acciones()}<Boton tam="pequeno" onclick={() => ((seccion = 'totp'), cambiarSeccion('totp'))}>Activar</Boton>{/snippet}
            </Aviso>
          {/if}
          <div class="pie">
            <span class="nota"><Icono nombre="info" tam={14} />Si tras aplicar dejas de ver el panel, espera 60 s y volverá como estaba.</span>
            <Boton
              variante="primario"
              cargando={aplicando}
              desactivado={!acceso || eleccion === acceso.alcance || !!pendiente}
              onclick={aplicarAcceso}>Aplicar</Boton
            >
          </div>
        </div>
      </Tarjeta>
    {/if}
  </div>
</div>

{#if pendiente}
  <Confirmacion60
    segundos={pendiente.segundos}
    mensaje={pendiente.segundos == null
      ? 'Hay un cambio de acceso sin confirmar. Confírmalo o se deshará solo.'
      : 'Acceso cambiado. Si sigues viendo el panel, confírmalo; si no, se deshará solo.'}
    onconfirmar={confirmarAcceso}
    onexpirar={expirado}
  />
{/if}

<style>
  .contenido {
    max-width: 760px;
  }
  .formulario {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .espacio {
    margin-bottom: 14px;
  }
  .dos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  .pie {
    display: flex;
    align-items: center;
    gap: 12px;
    justify-content: space-between;
    flex-wrap: wrap;
    padding-top: 4px;
  }
  .pie.fin {
    justify-content: flex-end;
  }
  .nota {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--texto-3);
    font-size: 12.5px;
  }
  .explica {
    color: var(--texto-2);
    line-height: 1.6;
  }
  .alta {
    display: flex;
    gap: 24px;
    align-items: flex-start;
  }
  .pasos {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .paso {
    display: flex;
    gap: 12px;
  }
  .paso p {
    color: var(--texto-2);
    font-size: 13px;
    margin: 2px 0 8px;
  }
  .num {
    width: 24px;
    height: 24px;
    flex: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--azul-tinte);
    color: var(--azul);
    font-size: 12px;
    font-weight: 700;
  }
  .crece {
    flex: 1;
    min-width: 0;
  }
  .secreto {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .secreto code {
    padding: 6px 10px;
    letter-spacing: 0.04em;
    overflow-wrap: anywhere;
    color: var(--texto);
  }
  .codigo {
    display: flex;
    gap: 8px;
    margin-top: 8px;
    max-width: 280px;
  }
  .cancelar {
    align-self: flex-start;
    background: none;
    border: 0;
    padding: 0;
    color: var(--texto-2);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }
  .cancelar:hover {
    color: var(--texto);
  }
  .opciones {
    display: grid;
    gap: 10px;
  }
  .opcion {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding: 14px 16px;
    background: var(--campo);
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: border-color 0.12s, background 0.12s;
  }
  .opcion:hover:not(:disabled) {
    border-color: var(--borde-fuerte);
  }
  .opcion.elegida {
    border-color: var(--azul);
    background: color-mix(in srgb, var(--azul-tinte) 45%, var(--campo));
  }
  .opcion:disabled {
    cursor: not-allowed;
  }
  .opcion:disabled .o-icono,
  .opcion:disabled .o-titulo,
  .opcion:disabled .o-desc {
    opacity: 0.55;
  }
  .radio {
    width: 18px;
    height: 18px;
    flex: none;
    margin-top: 2px;
    border-radius: 50%;
    border: 2px solid var(--borde-fuerte);
  }
  .elegida .radio {
    border-color: var(--azul);
    background: radial-gradient(circle, var(--azul) 0 4px, transparent 5px);
  }
  .o-icono {
    width: 38px;
    height: 38px;
    flex: none;
    border-radius: 9px;
    display: grid;
    place-items: center;
    background: var(--superficie-2);
    color: var(--texto-2);
  }
  .elegida .o-icono {
    background: var(--azul-tinte);
    color: var(--azul);
  }
  .o-texto {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .o-titulo {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
    flex-wrap: wrap;
  }
  .o-desc {
    color: var(--texto-2);
    font-size: 13px;
  }
  @media (max-width: 640px) {
    .dos {
      grid-template-columns: 1fr;
    }
    .alta {
      flex-direction: column;
      align-items: center;
    }
    .pie .nota {
      order: 2;
    }
  }
</style>
