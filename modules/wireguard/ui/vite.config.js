import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Bundle ES único (dist/index.js) que la carcasa carga con import('/m/wireguard/ui/index.js').
// Lleva dentro Svelte y el kit; el CSS de los componentes va inyectado en el JS. No carga
// tema.css: las variables globales ya las pone la carcasa.
export default defineConfig({
  plugins: [svelte({ emitCss: false, compilerOptions: { css: 'injected' } })],
  define: { 'process.env.NODE_ENV': JSON.stringify('production') },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    lib: {
      entry: 'src/index.js',
      formats: ['es'],
      fileName: () => 'index.js',
    },
    minify: true,
    chunkSizeWarningLimit: 600,
  },
});
