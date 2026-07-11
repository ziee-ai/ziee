// @ts-ignore
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'node:path'
import { formNamesPlugin } from './plugins/vite-plugin-form-names.js'
import { removeDataTestPlugin } from './plugins/vite-plugin-remove-data-test.js'
import { testidUniquePlugin } from './plugins/vite-plugin-testid-unique.js'
import { galleryCoveragePlugin } from './plugins/vite-plugin-gallery-coverage.js'
import { galleryAliasPlugin } from './plugins/vite-plugin-gallery-alias.js'

const host = process.env.TAURI_DEV_HOST

// https://vitejs.dev/config/
export default defineConfig(async () => {
  const isDev = process.env.NODE_ENV !== 'production'
  const isTest = process.env.NODE_ENV === 'test'

  // PART 2 (gallery branch-coverage): when GALLERY_COVERAGE=1, instrument the
  // component/page source with babel-plugin-istanbul so the render pass exposes
  // `window.__coverage__` (per-branch hit counts). Off by every other build —
  // normal dev/build/test never pays the instrumentation cost.
  const coverage = process.env.GALLERY_COVERAGE === '1'

  return {
    plugins: [
      // Instrument component/page source FIRST (enforce:'pre') so branch coverage
      // maps to real source lines, before react/oxc transpiles it.
      ...(coverage ? [galleryCoveragePlugin({ srcDir: path.resolve(__dirname, 'src') })] : []),
      react(),
      tailwindcss(),
      // Serve the gallery at the pretty `/gallery` URL + keep `/dev-gallery.html`
      // working post-rename (dev/preview only).
      galleryAliasPlugin(),
      // Detect duplicate form names
      formNamesPlugin({
        srcDir: 'src',
      }),
      // Fail the build on any duplicate data-testid literal (i18n-safe selectors)
      testidUniquePlugin({
        srcDirs: [path.resolve(__dirname, './src')],
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
    port: Number(process.env.VITE_DEV_PORT) || 1420,
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
        target: process.env.VITE_API_PROXY_TARGET || 'http://localhost:3000/',
        changeOrigin: true,
        // xfwd: http-proxy sets X-Forwarded-For/Port/Proto but NOT
        // X-Forwarded-Host. The backend's OAuth-authorize handler
        // derives redirect_uri from X-Forwarded-Host (the
        // user-facing origin); without it, Vite would proxy with
        // its target's HOST (the backend's internal port) and the
        // post-OAuth redirect would 404 against the backend port
        // instead of the SPA's port. We set X-Forwarded-Host
        // explicitly from the original request's Host header so
        // the backend always sees the user-facing origin.
        xfwd: true,
        configure: (proxy) => {
          proxy.on('proxyReq', (proxyReq, req) => {
            // Node's IncomingMessage typing allows `host` to be
            // string | string[] | undefined (multiple Host headers
            // arrive as an array — RFC 7230 §5.4 forbids this but
            // Node still parses them). Use the first value; if it
            // somehow stringified to "a,b" the backend's URL parse
            // would fail and 500.
            const raw = req.headers.host
            const host = Array.isArray(raw) ? raw[0] : raw
            if (host) {
              proxyReq.setHeader('X-Forwarded-Host', host)
            }
          })
        },
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
