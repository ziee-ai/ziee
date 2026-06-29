import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

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
    await byTestId(page, 'hardware-os-card').waitFor({ timeout: 30000 })

    // The auto-connected usage SSE delivers a live frame → the per-resource
    // usage progress bars render (they are conditional on `currentUsage`).
    await expect(byTestId(page, 'hardware-cpu-usage-progress')).toBeVisible({ timeout: 30000 })
    await expect(byTestId(page, 'hardware-memory-usage-progress')).toBeVisible({ timeout: 30000 })

    // A percentage value is rendered on the CPU progress (live numeric data).
    await expect(byTestId(page, 'hardware-cpu-usage-progress')).toContainText('%', {
      timeout: 30000,
    })
  })
})
