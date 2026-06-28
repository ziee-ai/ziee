import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — hardware monitoring CONNECT interaction. The existing hardware.spec.ts
 * is page-load smoke only (a11y + card presence) and never clicks anything; the
 * "Connect" button (handleManualConnect → Stores.Hardware.subscribeToHardwareUsage,
 * HardwareSettings.tsx:559-585) was never exercised.
 *
 * The hardware SSE (GET /api/hardware/usage-stream) is the external boundary —
 * mocked to fail so the page stays DISCONNECTED and the Connect button remains
 * visible/clickable deterministically. The button → handler → store wiring is
 * the behavior under test (the success toast confirms the connect attempt ran).
 */

test.describe('Hardware — manual connect interaction', () => {
  test('clicking Connect triggers a hardware-monitoring subscription', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Keep the page in the Disconnected state: the SSE endpoint errors, so
    // sseConnected never flips true and the Connect button stays rendered.
    await page.route(/\/api\/hardware\/usage-stream/, async route =>
      route.fulfill({ status: 500, contentType: 'text/plain', body: 'no stream in test' }),
    )

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/hardware`)
    await page.waitForSelector('text=Hardware', { timeout: 30000 })

    // The connection-status card shows Disconnected with a Connect button.
    await expect(page.getByText('Disconnected')).toBeVisible({ timeout: 30000 })
    const connect = page.getByRole('button', { name: 'Connect', exact: true })
    await expect(connect).toBeVisible({ timeout: 30000 })

    // Click it → handleManualConnect fires the subscribe + its success toast.
    await connect.click()
    await expect(
      page.getByText('Connecting to hardware monitoring...'),
    ).toBeVisible({ timeout: 10000 })
  })
})
