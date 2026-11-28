<script>
  import Boton from './Boton.svelte';
  /**
   * Asistente por pasos (máximo 3 en la tarea principal, §3b): «Paso 2 de 3», Atrás/Siguiente.
   * `pasos`: títulos. `puedeSeguir`: si el paso actual está completo. `ocupado`: al aplicar.
   * @type {{ pasos: string[], paso?: number, titulo?: string, frase?: string, puedeSeguir?: boolean,
   *   ocupado?: boolean, textoFin?: string, onfin?: () => void, oncancelar?: () => void,
   *   onsiguiente?: (paso: number) => void, children?: any }}
   */
  let {
    pasos,
    paso = $bindable(0),
    titulo = '',
    frase = '',
    puedeSeguir = true,
    ocupado = false,
    textoFin = 'Aplicar',
    onfin,
    oncancelar,
    onsiguiente,
    children,
  } = $props();
  const ultimo = $derived(paso === pasos.length - 1);
</script>

<div class="nc-asistente">
  {#if titulo}
    <header>
      <h2>{titulo}</h2>
      {#if frase}<p>{frase}</p>{/if}
    </header>
  {/if}
  <ol class="pasos">
    {#each pasos as p, i (i)}
      <li class:hecho={i < paso} class:actual={i === paso}>
        <span class="num">{i < paso ? '✓' : i + 1}</span>
        <span class="nombre">{p}</span>
      </li>
    {/each}
  </ol>
  <p class="contador">Paso {paso + 1} de {pasos.length}</p>
  <div class="cuerpo">{@render children?.()}</div>
  <footer>
    {#if oncancelar && paso === 0}
      <Boton variante="fantasma" onclick={oncancelar}>Cancelar</Boton>
    {:else if paso > 0}
      <Boton variante="fantasma" icono="chevron_izquierda" desactivado={ocupado} onclick={() => (paso -= 1)}>Atrás</Boton>
    {/if}
    <span class="hueco"></span>
    {#if ultimo}
      <Boton variante="primario" cargando={ocupado} desactivado={!puedeSeguir} onclick={() => onfin?.()}>{textoFin}</Boton>
    {:else}
      <Boton
        variante="primario"
        desactivado={!puedeSeguir}
        onclick={() => {
          onsiguiente?.(paso);
          paso += 1;
        }}>Siguiente</Boton
      >
    {/if}
  </footer>
</div>

<style>
  .nc-asistente {
    display: flex;
    flex-direction: column;
    gap: 18px;
    max-width: 760px;
    width: 100%;
    margin: 0 auto;
    padding: 24px;
    background: var(--superficie);
    border: 1px solid var(--borde);
    border-radius: var(--radio);
  }
  header h2 {
    font-size: 18px;
  }
  header p {
    margin: 6px 0 0;
    color: var(--texto-2);
  }
  .pasos {
    display: flex;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .pasos li {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    padding-top: 10px;
    border-top: 3px solid var(--borde);
    color: var(--texto-3);
    font-size: 12.5px;
  }
  .pasos li.actual {
    border-color: var(--azul);
    color: var(--texto);
  }
  .pasos li.hecho {
    border-color: var(--verde);
    color: var(--texto-2);
  }
  .num {
    width: 22px;
    height: 22px;
    flex: none;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: var(--superficie-2);
    font-weight: 600;
    font-size: 12px;
  }
  .actual .num {
    background: var(--azul);
    color: #fff;
  }
  .hecho .num {
    background: var(--verde-tinte);
    color: var(--verde);
  }
  .cuerpo {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .contador {
    margin: -8px 0 0;
    font-size: 12px;
    color: var(--texto-3);
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-top: 6px;
    border-top: 1px solid var(--borde-suave);
  }
  .hueco {
    flex: 1;
  }
  @media (max-width: 640px) {
    .nombre {
      display: none;
    }
  }
</style>
