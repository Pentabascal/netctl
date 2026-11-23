import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// La carcasa se incrusta en el binario del núcleo (rust-embed de web/shell/dist).
export default defineConfig({
  plugins: [svelte()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    assetsDir: 'assets',
    chunkSizeWarningLimit: 600,
  },
  server: {
    proxy: {
      '/api': { target: 'https://127.0.0.1:8443', secure: false },
      '/m/': { target: 'https://127.0.0.1:8443', secure: false },
    },
  },
});
