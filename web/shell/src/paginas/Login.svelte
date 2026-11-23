<script>
  import { onDestroy } from 'svelte';
  import { apiNucleo, fijarCsrf, Campo, Boton, Aviso } from '@netctl/kit';
  import { sesion, cargarYo } from '../sesion.svelte.js';

  let password = $state('');
  let totp = $state('');
  let pideTotp = $state(false);
  let error = $state('');
  let bloqueo = $state(0);
  let enviando = $state(false);
  let reloj = null;

  function contarBloqueo(s) {
    bloqueo = s;
    clearInterval(reloj);
    reloj = setInterval(() => {
      bloqueo = Math.max(0, bloqueo - 1);
      if (bloqueo === 0) clearInterval(reloj);
    }, 1000);
  }
  onDestroy(() => clearInterval(reloj));

  async function entrar(e) {
    e.preventDefault();
    if (!password || bloqueo) return;
    enviando = true;
    error = '';
    try {
      const r = await apiNucleo.post('/api/login', { password, totp: pideTotp ? totp : undefined });
      fijarCsrf(r.csrf);
      sesion.aviso = '';
      await cargarYo();
    } catch (err) {
      if (err.datos?.totp_requerido) {
        if (pideTotp && totp) error = 'El código de verificación no es correcto.';
        pideTotp = true;
        totp = '';
      } else if (err.estado === 429) {
        contarBloqueo(err.datos?.segundos ?? 60);
        error = '';
      } else {
        error = err.mensaje || 'No se pudo iniciar sesión.';
        if (err.estado === 401 && !pideTotp) password = '';
      }
    } finally {
      enviando = false;
    }
  }

  const minSeg = $derived(`${Math.floor(bloqueo / 60)}:${String(bloqueo % 60).padStart(2, '0')}`);
</script>

<div class="fondo">
  <div class="caja">
    <div class="marca">
      <svg viewBox="0 0 32 32" width="44" height="44" aria-hidden="true">
        <rect width="32" height="32" rx="9" fill="var(--azul)" />
        <g fill="none" stroke="#fff" stroke-width="2.2" stroke-linecap="round">
          <circle cx="16" cy="16" r="3" />
          <path d="M16 6v5M16 21v5M6 16h5M21 16h5" />
        </g>
      </svg>
      <h1>netctl</h1>
      <p>Inicia sesión para administrar la red</p>
    </div>

    <form class="tarjeta" onsubmit={entrar}>
      {#if sesion.aviso}
        <Aviso tipo="info">{sesion.aviso}</Aviso>
      {/if}
      {#if bloqueo}
        <Aviso tipo="aviso" titulo="Demasiados intentos">Espera {minSeg} antes de volver a probar.</Aviso>
      {:else if error}
        <Aviso tipo="error">{error}</Aviso>
      {/if}

      <Campo etiqueta="Usuario" valor="admin" soloLectura autocompletar="username" nombre="usuario" />
      {#if !pideTotp}
        <Campo
          etiqueta="Contraseña"
          tipo="password"
          bind:valor={password}
          autocompletar="current-password"
          nombre="password"
          enfocar
        />
      {:else}
        <div class="paso-totp">
          <p>Abre tu app de autenticación y escribe el código de 6 cifras de <strong>netctl</strong>.</p>
          <Campo
            etiqueta="Código de verificación"
            bind:valor={totp}
            inputmode="numeric"
            autocompletar="one-time-code"
            maxlength={6}
            placeholder="123456"
            mono
            enfocar
          />
        </div>
      {/if}

      <Boton variante="primario" tipo="submit" ancho cargando={enviando} desactivado={!password || bloqueo > 0 || (pideTotp && totp.length < 6)}>
        {pideTotp ? 'Verificar y entrar' : 'Entrar'}
      </Boton>
      {#if pideTotp}
        <button type="button" class="volver" onclick={() => ((pideTotp = false), (password = ''), (error = ''))}>
          Usar otra contraseña
        </button>
      {/if}
    </form>

    <p class="pie">
      ¿Has olvidado la contraseña? En el servidor ejecuta <code>netctl-core admin reset-password</code>
    </p>
  </div>
</div>

<style>
  .fondo {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 24px 16px;
    background:
      radial-gradient(900px 500px at 50% -10%, color-mix(in srgb, var(--azul) 14%, transparent), transparent 70%),
      var(--fondo);
  }
  .caja {
    width: min(380px, 100%);
    display: flex;
    flex-direction: column;
    gap: 22px;
  }
  .marca {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    text-align: center;
  }
  .marca h1 {
    font-size: 22px;
    margin-top: 4px;
  }
  .marca p {
    color: var(--texto-2);
  }
  .tarjeta {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 24px;
    background: var(--superficie);
    border: 1px solid var(--borde);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
  }
  .paso-totp {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .paso-totp p {
    color: var(--texto-2);
    font-size: 13px;
  }
  .volver {
    align-self: center;
    background: none;
    border: 0;
    color: var(--texto-2);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }
  .volver:hover {
    color: var(--texto);
  }
  .pie code {
    white-space: nowrap;
  }
  .pie {
    text-align: center;
    color: var(--texto-3);
    font-size: 12.5px;
    line-height: 1.7;
  }
</style>
