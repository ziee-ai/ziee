import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — real-time hardware monitoring flow (the existing hardware.spec only has
 * static page checks). An admin (who holds hardware::monitor) auto-connects the
 * usage SSE on mount; the CPU/Memory usage cards + "Last update" timestamp only
 * render once a live `currentUsage` frame arrives. Asserting they appear proves
 * the subscribe → SSE stream → store → render loop works end-to-end.
 */

test.describe('Hardware — real-time monitoring', () => {
  test('live CPU/Memory usage streams in and renders', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await page.goto(`${testInfra.baseURL}/settings/hardware`)
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

    // The auto-connected usage SSE delivers a live frame → the per-resource
    // usage sections render (they are conditional on `currentUsage`).
    await expect(page.getByText('CPU Usage')).toBeVisible({ timeout: 30000 })
    await expect(page.getByText('Memory Usage')).toBeVisible({ timeout: 30000 })

    // The live-data freshness line confirms a real timestamped frame arrived.
    await expect(page.getByText(/Last update:/)).toBeVisible({ timeout: 30000 })

    // A percentage value is rendered on the CPU progress (live numeric data).
    await expect(page.getByText(/\d+(\.\d+)?%/).first()).toBeVisible({
      timeout: 30000,
    })
  })
})
