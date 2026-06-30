import { defineConfig, devices } from '@playwright/test'

/**
 * Visual-testing Playwright config — Layers A (layout invariants) + B (screenshot
 * regression) over the component gallery.
 *
 * Distinct from `playwright.config.ts` (the feature E2E suite) in one decisive
 * way: the gallery is a **dev-only, backend-free** canvas, so this config boots
 * ONLY the Vite dev server (serving `/dev-gallery.html`) — no Postgres, no
 * `cargo run`, no global setup. That makes the visual layers fast and
 * deterministic.
 *
 * Run:
 *   npx playwright test -c playwright.visual.config.ts            # A + B
 *   npx playwright test -c playwright.visual.config.ts --list     # resolve only
 *   npx playwright test -c playwright.visual.config.ts --update-snapshots  # bless B
 */
const PORT = Number(process.env.GALLERY_PORT || 1420)
const BASE_URL = `http://localhost:${PORT}`

export default defineConfig({
  testDir: './tests/e2e/visual',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: process.env.PLAYWRIGHT_WORKERS
    ? Number(process.env.PLAYWRIGHT_WORKERS)
    : undefined,
  reporter: [['list'], ['html', { open: 'never' }]],

  // Bring up the gallery's Vite server. No backend needed — the standalone entry
  // registers only the ConfigClient store and renders the gallery under the real
  // ThemeProvider.
  webServer: {
    // Pass the port through to Vite (vite.config.ts pins strictPort), so the
    // GALLERY_PORT override actually works instead of hanging on :1420.
    command: `npm run dev -- --port ${PORT} --strictPort`,
    url: `${BASE_URL}/dev-gallery.html`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    stdout: 'ignore',
    stderr: 'pipe',
  },

  use: {
    baseURL: BASE_URL,
    trace: 'retain-on-failure',
  },

  // Screenshot determinism: disable animations + caret, soak up sub-pixel font
  // AA so unrelated machines don't false-fail.
  expect: {
    timeout: 10_000,
    toHaveScreenshot: {
      animations: 'disabled',
      caret: 'hide',
      // Text-dense sections (many tiny glyph edges) accumulate more font-AA jitter
      // per total pixel than large sections, so a tight ratio flakes red on the
      // SAME machine (observed ~0.03 on the tag section). 0.05 gives headroom.
      // Playwright ANDs the thresholds, so we do NOT add a maxDiffPixels floor
      // (it would make the gate stricter). True cross-machine stability still
      // requires blessing baselines in a pinned container (documented in README).
      maxDiffPixelRatio: 0.05,
      scale: 'css',
    },
  },

  projects: [
    {
      name: 'gallery',
      use: { ...devices['Desktop Chrome'] },
    },
  ],

  timeout: 60_000,
})
