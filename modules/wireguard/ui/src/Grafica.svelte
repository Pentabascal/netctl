<script>
  // Mini gráfica de área (últimos 5 min de tráfico), sin ejes: como las de las tarjetas de UniFi.
  /** @type {{ valores: number[], color?: string, alto?: number }} */
  let { valores = [], color = 'azul', alto = 36 } = $props();
  const ANCHO = 200;

  const caminos = $derived.by(() => {
    const v = valores.length >= 2 ? valores : [0, 0];
    const max = Math.max(...v, 1);
    const paso = ANCHO / (v.length - 1);
    const pts = v.map((y, i) => [i * paso, alto - 2 - (y / max) * (alto - 6)]);
    const linea = pts.map(([x, y], i) => `${i ? 'L' : 'M'}${x.toFixed(1)} ${y.toFixed(1)}`).join(' ');
    return { linea, area: `${linea} L${ANCHO} ${alto} L0 ${alto} Z` };
  });
</script>

<svg class="grafica {color}" viewBox="0 0 {ANCHO} {alto}" preserveAspectRatio="none" style:height="{alto}px" aria-hidden="true">
  <path class="area" d={caminos.area} />
  <path class="linea" d={caminos.linea} vector-effect="non-scaling-stroke" />
</svg>

<style>
  .grafica {
    display: block;
    width: 100%;
    margin-top: 10px;
  }
  .linea {
    fill: none;
    stroke: var(--c);
    stroke-width: 1.5;
    stroke-linejoin: round;
  }
  .area {
    fill: var(--c);
    opacity: 0.12;
  }
  .azul {
    --c: var(--azul);
  }
  .morado {
    --c: var(--morado);
  }
  .verde {
    --c: var(--verde);
  }
</style>
