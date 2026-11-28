<script>
  // Tarjetas de servicios habituales que rellenan puerto y protocolo; «Otro» para ponerlos a mano.
  import { Icono, Campo, CampoNumero } from '@netctl/kit';
  import { textoPuertos, PROTOCOLOS } from './estado.svelte.js';

  /** @type {{ plantillas: any[], plantilla?: string, puertos?: string, protocolo?: string, valido?: boolean }} */
  let { plantillas, plantilla = $bindable(''), puertos = $bindable(''), protocolo = $bindable('tcp'), valido = $bindable(false) } = $props();

  let puertosValidos = $state(true);
  const ICONOS = { web: 'globo', juego: 'actividad', multimedia: 'ojo', camara: 'ojo', terminal: 'terminal', pantalla: 'servidor', carpeta: 'caja', correo: 'enlace', base_datos: 'caja', mas: 'mas' };

  function elegir(p) {
    plantilla = p.id;
    if (p.id !== 'otro') {
      puertos = textoPuertos(p.puertos);
      protocolo = p.protocolo;
    } else {
      puertos = '';
      protocolo = 'tcp_udp';
    }
  }

  $effect(() => {
    valido = !!plantilla && !!puertos.trim() && puertosValidos;
  });
</script>

<div class="rejilla" role="radiogroup" aria-label="Servicio">
  {#each plantillas as p (p.id)}
    <button type="button" role="radio" aria-checked={plantilla === p.id} class:elegida={plantilla === p.id} onclick={() => elegir(p)}>
      <Icono nombre={ICONOS[p.icono] ?? 'red'} tam={20} />
      <b>{p.nombre}</b>
      <small>{p.id === 'otro' ? 'Pon el puerto tú' : `${textoPuertos(p.puertos)} · ${p.protocolo === 'tcp_udp' ? 'TCP y UDP' : p.protocolo.toUpperCase()}`}</small>
      {#if p.delicado}<span class="delicado" title="Peligroso abierto a todo internet">!</span>{/if}
    </button>
  {/each}
</div>

{#if plantilla === 'otro'}
  <div class="dos">
    <CampoNumero
      etiqueta="Puerto o rango"
      bind:valor={puertos}
      bind:valido={puertosValidos}
      puertos
      requerido
      placeholder="25565 o 27015-27030"
      ayuda="Mira en las instrucciones del programa qué puerto usa."
      mas={{ ejemplo: '25565  ·  27015-27030  ·  80, 443' }}
    />
    <Campo etiqueta="Protocolo" tipo="select" bind:valor={protocolo} opciones={PROTOCOLOS} ayuda="Si no lo sabes, deja TCP y UDP." />
  </div>
{/if}

<style>
  .rejilla {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 8px;
  }
  button {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 12px;
    text-align: left;
    background: var(--superficie-2);
    border: 1px solid var(--borde);
    border-radius: var(--radio);
    color: var(--texto-2);
    font: inherit;
    cursor: pointer;
  }
  button:hover {
    border-color: var(--borde-fuerte);
  }
  button.elegida {
    border-color: var(--azul);
    background: var(--azul-tinte);
    color: var(--azul);
  }
  b {
    color: var(--texto);
    font-weight: 500;
    font-size: 13px;
  }
  small {
    color: var(--texto-3);
    font-size: 11.5px;
    font-family: var(--mono);
  }
  .delicado {
    position: absolute;
    top: 8px;
    right: 10px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--ambar-tinte);
    color: var(--ambar);
    font-size: 11px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .dos {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    margin-top: 12px;
  }
</style>
