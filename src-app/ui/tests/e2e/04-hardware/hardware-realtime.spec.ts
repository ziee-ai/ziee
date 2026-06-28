import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * Real-time hardware monitoring (SSE) flow.
 *
 * The existing 04-hardware specs are static page checks. This drives the LIVE
 * path: the `/hardware-monitor` route mounts <HardwareMonitor>, whose
 * `useEffect` calls `Stores.Hardware.subscribeToHardwareUsage()` — opening the
 * `/api/hardware/usage` SSE stream. The CPU/Memory usage cards + the
 * "Last update:" line render ONLY once a `currentUsage` frame arrives over that
 * stream, so their appearance proves the end-to-end SSE store integration
 * (subscribe → receive → render), not just a static page.
 */
test.describe('Hardware - real-time SSE monitoring', () => {
  test('hardware-monitor page receives live usage frames over SSE', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // The Monitor button opens this route in a popup; navigate to it directly.
    await page.goto(`${baseURL}/hardware-monitor`)

    // These cards are gated on `currentUsage` (the first SSE frame). Their
    // visibility is the proof that the store subscribed and received data.
    await expect(page.getByText('CPU Usage')).toBeVisible({ timeout: 30000 })
    await expect(page.getByText('Memory Usage')).toBeVisible({ timeout: 30000 })

    // The "Last update:" timestamp is rendered from the received frame's
    // `timestamp`, confirming a real frame (not just a mounted shell).
    await expect(page.getByText(/Last update:/)).toBeVisible({ timeout: 30000 })
  })
})
