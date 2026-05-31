/**
 * Real-backend smoke test for desktop.
 *
 * Unlike the other desktop specs (which use `installTauriMock` with
 * canned tokens + `mockBackendDefaults` to block real API calls),
 * this spec uses the `testInfra` fixture from
 * `../fixtures/test-context`. The fixture spawns `cargo run --bin
 * ziee` on a per-worker port, creates a fresh Postgres DB, bootstraps
 * an admin user, and logs in for real JWT tokens.
 *
 * We then wire those into `installTauriMock(page, { backendPort,
 * tokens })` and do NOT call `mockBackendDefaults`. The SPA's
 * `getBaseUrl()` resolves to the spawned backend's port; every
 * `fetch('/api/...')` actually hits it.
 *
 * Cold-build cost: first run waits for `cargo run --bin ziee` to
 * compile (60-90s). Subsequent runs hit cargo's incremental cache
 * (~5-10s). Pre-warm with `cd src-app && cargo build -p ziee` for
 * fast first runs.
 */

import { test, expect } from '../fixtures/test-context'
import { installTauriMock } from './helpers/tauri-mock'

test.describe('desktop real-backend smoke', () => {
  test('loads the app + reaches the real backend on the per-test port', async ({
    page,
    testInfra,
  }) => {
    // Sanity-check the fixture handed us a live backend.
    expect(testInfra.backendPort).toBeGreaterThan(9000)
    expect(testInfra.tokens.user.username).toBe('admin')
    expect(testInfra.tokens.user.is_admin).toBe(true)
    expect(testInfra.tokens.access_token).toBeTruthy()

    // Direct backend probe (bypasses the browser) — confirms the
    // spawned cargo process answers /api/health on the locked port.
    const health = await fetch(`${testInfra.backendURL}/api/health`)
    expect(health.ok).toBe(true)

    // Inject the Tauri shim so the SPA goes through auto-login as if
    // it were running inside the desktop bundle. Note: NO
    // `mockBackendDefaults` — we want real /api requests to land.
    await installTauriMock(page, {
      backendPort: testInfra.backendPort,
      tokens: testInfra.tokens,
    })

    // Register the response listener BEFORE navigation so we catch
    // the SPA's earliest API calls (the App store fires
    // /api/app/setup/status on mount).
    const responses: number[] = []
    page.on('response', res => {
      if (res.url().includes(`127.0.0.1:${testInfra.backendPort}/api/`)) {
        responses.push(res.status())
      }
    })

    await page.goto('/')

    // Once auto-login completes, the SPA leaves the bootstrap spinner.
    // Watching for the "Starting up…" text to DISAPPEAR is the
    // contract; what comes next depends on whether onboarding fires.
    await expect(page.getByText(/starting up/i)).toBeHidden({
      timeout: 15_000,
    })
    await expect(
      page.getByRole('textbox', { name: /username/i }),
    ).toHaveCount(0)

    // Give the SPA a beat to make its initial API calls.
    await page.waitForTimeout(2_000)

    expect(responses.length).toBeGreaterThan(0)
    // None of those startup calls should 5xx — that'd indicate a
    // broken backend / migration / config.
    expect(responses.every(s => s < 500)).toBe(true)
  })
})
