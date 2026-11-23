<script>
  import qrcode from 'qrcode-generator';
  /**
   * Dibuja un código QR en el navegador (sin servicios externos). Fondo blanco para que lo lea
   * cualquier cámara aunque el tema sea oscuro.
   * @type {{ texto: string, tam?: number, correccion?: 'L'|'M'|'Q'|'H' }}
   */
  let { texto, tam = 200, correccion = 'M' } = $props();

  const svg = $derived.by(() => {
    if (!texto) return '';
    const qr = qrcode(0, correccion);
    qr.addData(texto, 'Byte');
    qr.make();
    const n = qr.getModuleCount();
    let d = '';
    for (let y = 0; y < n; y++)
      for (let x = 0; x < n; x++) if (qr.isDark(y, x)) d += `M${x + 4} ${y + 4}h1v1h-1z`;
    const lado = n + 8;
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${lado} ${lado}" shape-rendering="crispEdges"><rect width="${lado}" height="${lado}" fill="#fff"/><path d="${d}" fill="#000"/></svg>`;
  });
</script>

<div class="nc-qr" style:width="{tam}px" style:height="{tam}px" role="img" aria-label="Código QR">{@html svg}</div>

<style>
  .nc-qr {
    border-radius: 8px;
    overflow: hidden;
    background: #fff;
    flex: none;
  }
  .nc-qr :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
