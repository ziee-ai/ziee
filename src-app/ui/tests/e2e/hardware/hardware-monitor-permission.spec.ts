import { test, expect } from '../../fixtures/test-context'
import { loginWithPerms } from '../permissions/fixtures'
import { byTestId } from '../testid'
import { Permissions } from '../../../src/api-client/permissions'

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
    await byTestId(page, 'hardware-os-card').waitFor({ timeout: 30000 })

    // The static monitoring status still renders (the tag is permission-agnostic)…
    await expect(byTestId(page, 'hardware-settings-connection-tag')).toBeVisible({
      timeout: 15000,
    })
    // …but the Connect control is hidden for a non-monitor viewer.
    await expect(byTestId(page, 'hardware-settings-connect-btn')).toHaveCount(0)
  })
})
