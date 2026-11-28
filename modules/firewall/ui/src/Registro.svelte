<script>
  // Firewall › Registro: lo que se ha aplicado (y si se confirmó o se deshizo solo) y, en
  // solo lectura, las reglas nftables que se generan.
  import { onMount } from 'svelte';
  import { Barra, Pestanas, Registro as ListaRegistro, ConfigGenerada, Aviso } from '@netctl/kit';
  import { fw, textoError } from './estado.svelte.js';

  let { api } = $props();
  let pestana = $state('cambios');
  let entradas = $state([]);
  let texto = $state('');
  let error = $state('');

  async function cargar() {
    try {
      entradas = (await api.get('/registro')).entradas;
      texto = (await api.get('/generada')).texto;
    } catch (e) {
      error = textoError(e);
    }
  }
  onMount(cargar);
  $effect(() => {
    fw.version;
    cargar();
  });
</script>

<div class="nc-pagina">
  <Barra titulo="Registro" subtitulo="Qué ha cambiado en el firewall, quién lo hizo y cómo terminó." icono="reloj" />
  <Pestanas
    pestanas={[
      { id: 'cambios', titulo: 'Cambios', contador: entradas.length },
      { id: 'generada', titulo: 'Configuración generada' },
    ]}
    bind:activa={pestana}
  />
  {#if error}<Aviso tipo="error">{error}</Aviso>{/if}
  {#if pestana === 'cambios'}
    <ListaRegistro {entradas} />
  {:else}
    <ConfigGenerada {texto} frase="Las reglas que netctl carga en el sistema (tabla nftables «inet netctl»). Se generan solas; no hace falta tocarlas." />
  {/if}
</div>
