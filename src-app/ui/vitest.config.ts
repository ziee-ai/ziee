/**
 * Vitest config for the main UI workspace's *store* unit tests.
 *
 * The bulk of this workspace's unit specs run under `node:test` (see
 * `npm run test:unit` + `scripts/node-test-loader.mjs`) and test pure,
 * extracted helpers. The voice model-management stores, however, need module
 * mocking (`vi.mock` for `@/api-client` + the SSE/XHR seams) and a DOM
 * (`window.setTimeout`, jsdom) to exercise the Zustand store lifecycle
 * end-to-end — so they run under Vitest, mirroring the desktop UI workspace's
 * `vitest.config.ts` store-test pattern.
 *
 * `include` is scoped to `*.store.test.ts` and the node:test-authored
 * `*.store.test.ts` files are excluded, so `npx vitest run` never double-runs
 * a `node:test` spec.
 */
import path from 'node:path'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  test: {
    environment: 'jsdom',
    globals: false,
    include: ['src/**/*.store.test.ts'],
    exclude: [
      'node_modules',
      'dist',
      'tests/e2e',
      // Authored against `node:test`, not Vitest — run via `npm run test:unit`.
      'src/modules/chat/core/stores/MessageViewState.store.test.ts',
    ],
    // Each file gets its own module graph so the singleton Zustand stores don't
    // bleed state (or module-scope SSE/XHR handles) across files.
    isolate: true,
  },
})
