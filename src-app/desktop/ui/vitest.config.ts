/**
 * Vitest config for the desktop UI workspace.
 *
 * Mirrors the Vite path aliases so `@ziee/desktop/*` and `@/*` resolve
 * identically to the production bundle. Uses jsdom so the Zustand
 * stores can run their normal lifecycle (subscribe, persist, etc.)
 * without crashing on `document`/`localStorage` access.
 *
 * Only picks up files under `src/**\/*.test.ts` — Playwright specs
 * under `tests/e2e/` are explicitly excluded so a misnamed
 * `*.test.ts` next to a `*.spec.ts` doesn't collide.
 */

import { defineConfig } from 'vitest/config'
import path from 'path'

export default defineConfig({
  resolve: {
    alias: {
      '@ziee/desktop': path.resolve(__dirname, './src'),
      '@ziee/ui-core': path.resolve(__dirname, '../../ui/src'),
      '@': path.resolve(__dirname, '../../ui/src'),
    },
  },
  test: {
    environment: 'jsdom',
    globals: false,
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
    exclude: ['node_modules', 'dist', 'tests/e2e'],
    setupFiles: ['./vitest.setup.ts'],
    // Each test file gets its own module graph so the singleton
    // Zustand stores don't bleed across files.
    isolate: true,
  },
})
