/**
 * Playwright config for the DESKTOP gallery specs (backend-free).
 *
 * The standard `playwright.config.ts` spins a Postgres backend via global-setup
 * for the real-backend desktop specs. The gallery specs run entirely against the
 * mock-API cassette (`/gallery.html`), so this config drops global-setup + the
 * backend and just boots the gallery Vite dev server. Mirrors the web
 * workspace's separate `playwright.visual.config.ts`.
 */
import { defineConfig, devices } from '@playwright/test'

const PORT = Number(process.env.GALLERY_PORT || 1455)

export default defineConfig({
  testDir: './tests/e2e',
  testMatch: /gallery-desktop-.*\.spec\.ts$/,
  fullyParallel: false,
  workers: 1,
  timeout: 60_000,
  reporter: [['list']],
  use: {
    baseURL: `http://localhost:${PORT}`,
    trace: 'off',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 720 } },
    },
  ],
  webServer: {
    command: `npm run dev -- --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}/gallery.html`,
    reuseExistingServer: true,
    timeout: 120_000,
  },
})
