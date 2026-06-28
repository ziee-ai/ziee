import { test, expect } from '../../fixtures/test-context'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/types'

/**
 * E2E — `hardware::monitor` permission gating of the live-monitoring controls.
 *
 * `/settings/hardware` requires `hardware::read` to view, but the live
 * "Connect" button (and the auto-connect SSE stream) is additionally gated on
 * `hardware::monitor` (`canMonitor = usePermission(HardwareMonitor)`;
 * `HardwareSettings.tsx:581-584`). A read-only viewer must see the static
 * hardware card WITHOUT a Connect button.
 */

test.describe('Hardware — monitor-permission gating', () => {
  test('a read-only (no monitor) user sees the card but no Connect button', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // hardware::read but deliberately NOT hardware::monitor.
    await loginWithPerms(page, baseURL, apiURL, [Permissions.HardwareRead])

    await page.goto(`${baseURL}/settings/hardware`)
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

    // The static monitoring status still renders (the tag is permission-agnostic)…
    await expect(page.getByText('Real-time Monitoring:')).toBeVisible({
      timeout: 15000,
    })
    // …but the Connect control is hidden for a non-monitor viewer.
    await expect(
      page.getByRole('button', { name: 'Connect', exact: true }),
    ).toHaveCount(0)
  })
})
