<script>
  // Pestaña «Ajustes generales»: ping desde internet y salida a internet (NAT) por zona.
  import { Opcion, Aviso } from '@netctl/kit';
  import { aplicado, textoError } from './estado.svelte.js';

  let { api, config } = $props();
  let error = $state('');
  const c = $derived(config.config);
  const conSalida = $derived(c.zonas.filter((z) => !['externa', 'gateway'].includes(z.tipo)));

  async function poner(cambio) {
    error = '';
    try {
      aplicado(await api.put('/ajustes', { ...cambio, revision: c.revision }));
    } catch (e) {
      error = textoError(e);
    }
  }
</script>

<p class="frase">Opciones que afectan a todo el firewall.</p>

<div class="bloque">
  <Opcion
    titulo="Responder a ping desde internet (recomendado)"
    texto="Deja comprobar desde fuera si la máquina está encendida. No abre ningún servicio."
    activo={c.ping_externo}
    onchange={(v) => poner({ ping_externo: v })}
    mas={{ texto: 'El ping (ICMP «echo») sirve para diagnosticar. Ocultarlo apenas protege.', recomendado: 'activado' }}
  />
</div>

<h3 class="nc-seccion">Salida a internet (NAT)</h3>
<p class="frase">
  Para que los equipos de una zona salgan a internet con la IP de esta máquina (enmascaramiento, «masquerade»). Si lo
  apagas, esa zona solo llegará a internet si otro router hace el NAT.
</p>
<div class="bloque">
  {#each conSalida as z (z.id)}
    <Opcion
      titulo="{z.nombre} sale a internet con la IP de esta máquina{['interna', 'vpn', 'dmz'].includes(z.tipo) ? ' (recomendado)' : ''}"
      texto={z.tipo === 'vpn' ? 'Los clientes VPN con «Internet a través de la VPN» lo necesitan.' : 'Lo normal si esta máquina es el router de esa red.'}
      activo={!!c.nat[z.id]}
      onchange={(v) => poner({ nat: { [z.id]: v } })}
    />
  {/each}
</div>
{#if error}<Aviso tipo="error">{error}</Aviso>{/if}

<style>
  .frase {
    margin: 0;
    color: var(--texto-2);
  }
  .bloque {
    padding: 4px 16px;
    background: var(--superficie);
    border: 1px solid var(--borde);
    border-radius: var(--radio);
  }
</style>
