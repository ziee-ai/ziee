// @ts-ignore
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'node:path'
import { formNamesPlugin } from './plugins/vite-plugin-form-names.js'
import { removeDataTestPlugin } from './plugins/vite-plugin-remove-data-test.js'

const host = process.env.TAURI_DEV_HOST

// https://vitejs.dev/config/
export default defineConfig(async () => {
  const isDev = process.env.NODE_ENV !== 'production'
  const isTest = process.env.NODE_ENV === 'test'

  return {
    plugins: [
      react(),
      tailwindcss(),
      // Detect duplicate form names
      formNamesPlugin({
        srcDir: 'src',
      }),
      // Remove data-test-* attributes in production builds
      ...(isDev || isTest ? [] : [removeDataTestPlugin()]),
    ],

  // Vite options tailored for Tauri development
  clearScreen: false,
  root: 'src',

  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },

  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: [
        '**/.*/**',
        '**/node_modules/**',
      ],
    },
    proxy: {
      '/api/': {
        target: 'http://localhost:3000/',
        changeOrigin: true,
        // xfwd: forward X-Forwarded-* headers so the backend can
        // build absolute URLs (OAuth redirect_uri, callback hops)
        // that point back through the Vite proxy rather than at the
        // backend's internal port. Without this, post-OAuth redirects
        // land on the backend's port and 404 (the SPA isn't served
        // there in dev).
        xfwd: true,
      },
    },
    allowedHosts: true
  },

  build: {
    outDir: '../../dist/ui',
  },

  // NOTE: stripping `console.log` from prod bundles is deferred. Vite 8
  // uses Rolldown's Oxc minifier by default, which doesn't honor the
  // esbuild `pure` config. Forcing `build.minify: 'esbuild'` errors
  // out on Vite 8. The Biome `noConsole` rule (biome.json) prevents
  // NEW console.log/debug additions; pre-existing 173 calls remain in
  // the bundle as verbose noise but no security/correctness impact.
  // Follow-up: install vite-plugin-remove-console or pin Rolldown's
  // minifier options. (audit 09 B-15)

  }
})
