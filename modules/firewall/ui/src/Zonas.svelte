<script>
  // Firewall › Zonas: tarjetas arriba y pestañas Matriz / Interfaces / Ajustes. Con una sola
  // tarjeta de red, en lugar de la matriz se ve «Qué puede entrar a esta máquina».
  import { onMount } from 'svelte';
  import { Barra, Pestanas, Tarjeta, Matriz, Aviso } from '@netctl/kit';
  import { fw } from './estado.svelte.js';
  import PanelCasilla from './PanelCasilla.svelte';
  import Interfaces from './Interfaces.svelte';
  import Ajustes from './Ajustes.svelte';
  import SoloEntrada from './SoloEntrada.svelte';

  let { api, config } = $props();

  let pestana = $state('matriz');
  let casilla = $state(null); // { origen, destino }
  let resumen = $state(null);

  async function cargarResumen() {
    try {
      resumen = await api.get('/resumen');
    } catch {
      resumen = null;
    }
  }
  onMount(() => {
    cargarResumen();
    const t = setInterval(cargarResumen, 5000);
    return () => clearInterval(t);
  });
  $effect(() => {
    fw.version;
    cargarResumen();
  });

  const c = $derived(config.config);
  // Orden de la matriz: Externa, Interna, VPN, DMZ, personalizadas y la propia máquina al final.
  const ORDEN = { externa: 0, interna: 1, vpn: 2, dmz: 3, personalizada: 4, gateway: 5 };
  const zonas = $derived([...c.zonas].sort((a, b) => ORDEN[a.tipo] - ORDEN[b.tipo]));
  const celdas = $derived(Object.fromEntries(config.matriz.map((m) => [`${m.origen}>${m.destino}`, m])));
  const metrica = (n) => resumen?.metricas?.find((m) => m.etiqueta === n);
</script>

<div class="nc-pagina">
  <Barra
    titulo={config.una_interfaz ? 'Qué puede entrar a esta máquina' : 'Zonas'}
    subtitulo={config.una_interfaz
      ? 'Esta máquina tiene una sola tarjeta de red: aquí eliges qué servicios se pueden usar desde fuera.'
      : 'Cada tarjeta de red pertenece a una zona. En la matriz decides qué zona puede hablar con cuál.'}
    icono="escudo"
  />

  <div class="tarjetas">
    <Tarjeta etiqueta="Tarjetas de red" icono="red" valor={Object.keys(c.interfaces).length} detalle="{c.zonas.length} zonas" />
    <Tarjeta etiqueta="Reglas activas" icono="cortafuegos" valor={c.reglas.filter((r) => r.activa).length} detalle="Además de las fijas" />
    <Tarjeta etiqueta="Bloqueados" icono="escudo" valor={metrica('Bloqueados')?.valor ?? '—'} detalle="Paquetes desde el último arranque" />
  </div>

  {#if config.una_interfaz}
    <SoloEntrada {api} {config} />
  {:else}
    <Pestanas
      pestanas={[
        { id: 'matriz', titulo: 'Matriz' },
        { id: 'interfaces', titulo: 'Tarjetas y zonas', contador: Object.keys(c.interfaces).length },
        { id: 'ajustes', titulo: 'Ajustes generales' },
      ]}
      bind:activa={pestana}
    />
    {#if pestana === 'matriz'}
      <p class="frase">
        Pulsa una casilla para cambiar lo que pasa con ese tráfico o añadir excepciones. Las filas son de dónde sale
        el tráfico y las columnas, a dónde va.
      </p>
      <Matriz
        {zonas}
        celda={(o, d) => celdas[`${o}>${d}`]}
        elegida={casilla ? `${casilla.origen}>${casilla.destino}` : ''}
        onelegir={(o, d) => (casilla = { origen: o, destino: d })}
      />
      <div class="leyenda">
        <span><i class="v"></i> Permitir</span>
        <span><i class="r"></i> Bloquear</span>
        <span><b class="num">2</b> reglas propias</span>
        <span><b class="punto">•</b> cambiada respecto a la de serie</span>
      </div>
      {#if c.zonas.some((z) => z.tipo === 'vpn')}
        <Aviso tipo="info">
          La VPN tiene dos llaves: aquí decides qué puede hacer la zona VPN entera, y en
          <a href="/modulo/wireguard/clientes">WireGuard › Clientes</a> qué puede hacer cada cliente. El tráfico tiene
          que estar permitido en los dos sitios.
        </Aviso>
      {/if}
    {:else if pestana === 'interfaces'}
      <Interfaces {api} {config} />
    {:else}
      <Ajustes {api} {config} />
    {/if}
  {/if}
</div>

{#if casilla}
  <PanelCasilla {api} {config} origen={casilla.origen} destino={casilla.destino} oncerrar={() => (casilla = null)} />
{/if}

<style>
  .tarjetas {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 12px;
  }
  .frase {
    margin: 0;
    color: var(--texto-2);
  }
  .leyenda {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
    font-size: 12px;
    color: var(--texto-2);
  }
  .leyenda span {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .leyenda i {
    width: 12px;
    height: 12px;
    border-radius: 3px;
  }
  .leyenda .v {
    background: var(--verde-tinte);
    border: 1px solid var(--verde);
  }
  .leyenda .r {
    background: var(--rojo-tinte);
    border: 1px solid var(--rojo);
  }
  .num {
    min-width: 18px;
    height: 18px;
    border-radius: 9px;
    background: var(--superficie-2);
    font-size: 11px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .punto {
    color: var(--azul);
  }
</style>
