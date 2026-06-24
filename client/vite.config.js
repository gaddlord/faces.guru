import { defineConfig } from 'vite';

// `base: './'` makes built asset paths relative, which is required when the bundle
// is loaded from the Capacitor webview (capacitor://localhost / file://).
export default defineConfig({
  base: './',
  build: {
    outDir: 'dist',
    target: 'es2020',
  },
  server: {
    host: true,
    port: 5173,
  },
});
