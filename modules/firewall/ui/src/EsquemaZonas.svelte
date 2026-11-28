<script>
  // Esquema plegable de arriba (UniFi): Internet → Esta máquina → las zonas de dentro, cada una
  // con sus tarjetas de red.
  import { Icono } from '@netctl/kit';

  let { zonas, tarjetas } = $props();

  const ICONO = { externa: 'globo', gateway: 'servidor', interna: 'red', vpn: 'escudo', hotspot: 'wifi', dmz: 'caja', personalizada: 'casa_red' };
  const AYUDA = {
    externa: 'Internet y todo lo que llega de fuera.',
    gateway: 'Esta máquina: el propio servidor y sus servicios.',
    interna: 'Tus equipos de confianza.',
    vpn: 'Dispositivos conectados por la VPN.',
    hotspot: 'Invitados: solo salen a internet.',
    dmz: 'Servidores expuestos que no deben ver tu red.',
    personalizada: 'Zona creada por ti.',
  };

  const leer = () => {
    try {
      return localStorage.getItem('netctl.fw.esquema') !== 'plegado';
    } catch {
      return true;
    }
  };
  let abierto = $state(leer());
  function alternar() {
    abierto = !abierto;
    try {
      localStorage.setItem('netctl.fw.esquema', abierto ? 'abierto' : 'plegado');
    } catch {
      /* sin almacenamiento: da igual */
    }
  }

  const de = (tipo) => zonas.filter((z) => z.tipo === tipo);
  const dentro = $derived(zonas.filter((z) => !['externa', 'gateway'].includes(z.tipo)));
</script>

{#snippet caja(z)}
  <div class="zona" title={AYUDA[z.tipo]}>
    <span class="nombre">{z.nombre}</span>
    <div class="cajas">
      {#each tarjetas[z.id]?.length ? tarjetas[z.id] : [null] as t, i (t ?? i)}
        <span class="tarjeta" class:vacia={!t && z.tipo !== 'gateway'}>
          <Icono nombre={ICONO[z.tipo]} tam={18} />
          <span>{t ?? (z.tipo === 'gateway' ? 'Este servidor' : 'Sin tarjetas')}</span>
        </span>
      {/each}
    </div>
  </div>
{/snippet}

<section class="esquema" class:abierto>
  <button type="button" class="plegar" aria-expanded={abierto} aria-label={abierto ? 'Plegar el esquema' : 'Ver el esquema'} onclick={alternar}>
    <Icono nombre={abierto ? 'flecha_arriba' : 'flecha_abajo'} tam={14} />
  </button>
  {#if abierto}
    <div class="dibujo">
      {#each de('externa') as z (z.id)}{@render caja(z)}{/each}
      <span class="flecha" aria-hidden="true"></span>
      {#each de('gateway') as z (z.id)}{@render caja(z)}{/each}
      <span class="flecha" aria-hidden="true"></span>
      <div class="dentro">
        {#each dentro as z (z.id)}{@render caja(z)}{/each}
      </div>
    </div>
  {/if}
  <p>
    Las zonas agrupan tus redes para poner políticas generales en lugar de muchas reglas sueltas. Así es más fácil de
    entender, más seguro y más fácil de vigilar.
  </p>
</section>

<style>
  .esquema {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px 44px 16px 16px;
    background: var(--superficie);
    border: 1px solid var(--borde-suave);
    border-radius: var(--radio);
  }
  .plegar {
    position: absolute;
    top: 10px;
    right: 10px;
    background: none;
    border: 0;
    color: var(--texto-2);
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    cursor: pointer;
  }
  .dibujo {
    display: flex;
    align-items: flex-end;
    flex-wrap: wrap;
    gap: 6px;
  }
  .dentro {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .zona {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .nombre {
    font-size: 12px;
    color: var(--texto-2);
  }
  .cajas {
    display: flex;
    gap: 2px;
    padding: 6px 8px;
    border: 1px dashed var(--azul);
    border-radius: 4px;
    background: var(--azul-tinte);
  }
  .tarjeta {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    min-width: 70px;
    font-size: 11.5px;
    color: var(--azul);
  }
  .tarjeta span {
    color: var(--texto-2);
  }
  .tarjeta.vacia {
    opacity: 0.55;
  }
  .flecha {
    align-self: center;
    margin-top: 18px;
    width: 26px;
    height: 1px;
    background: var(--azul);
    position: relative;
  }
  .flecha::after {
    content: '';
    position: absolute;
    right: -1px;
    top: -3px;
    border: 3.5px solid transparent;
    border-left-color: var(--azul);
  }
  p {
    color: var(--texto-2);
    max-width: 760px;
  }
</style>
