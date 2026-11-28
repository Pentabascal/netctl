<script>
  // «Ajustes del firewall»: ping desde internet y salida a internet (NAT) por zona.
  import { PanelLateral, Bloque, Opcion, Aviso } from '@netctl/kit';
  import { aplicado, textoError } from './estado.svelte.js';

  let { api, config, oncerrar } = $props();
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

<PanelLateral abierto titulo="Ajustes del firewall" {oncerrar}>
  <Bloque titulo="Ping" ayuda="El ping (ICMP «echo») sirve para comprobar si la máquina está encendida. Ocultarlo apenas protege.">
    <Opcion
      titulo="Responder a ping desde internet (recomendado)"
      texto="Deja comprobar desde fuera si la máquina está encendida. No abre ningún servicio."
      activo={c.ping_externo}
      onchange={(v) => poner({ ping_externo: v })}
    />
  </Bloque>
  <Bloque
    titulo="Salida a internet (NAT)"
    ayuda="Para que los equipos de una zona salgan a internet con la IP de esta máquina (enmascaramiento, «masquerade»). Si lo apagas, esa zona solo llegará a internet si otro router hace el NAT."
  >
    {#each conSalida as z (z.id)}
      <Opcion
        titulo="{z.nombre} sale a internet con la IP de esta máquina{['interna', 'vpn', 'dmz', 'hotspot'].includes(z.tipo) ? ' (recomendado)' : ''}"
        texto={z.tipo === 'vpn' ? 'Los clientes VPN con «Internet a través de la VPN» lo necesitan.' : 'Lo normal si esta máquina es el router de esa red.'}
        activo={!!c.nat[z.id]}
        onchange={(v) => poner({ nat: { [z.id]: v } })}
      />
    {/each}
  </Bloque>
  {#if error}<Aviso tipo="error">{error}</Aviso>{/if}
</PanelLateral>
