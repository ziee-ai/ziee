import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

// audit id all-c72d44950b93 — the hardware error state was never triggered:
// HardwareSettings.tsx, when there is no hardware info to show, renders a
// persistent, retryable <ErrorState> (the durable error surface). The toast is
// now gated to a refresh failure (hardwareInfo already present), so an
// initial-load failure surfaces the ErrorState only. We force GET /api/hardware
// to fail so it appears.
test.describe('Hardware settings error state', () => {
  test('a failed hardware fetch shows the persistent error state', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Fail the hardware info request → store sets hardwareError, no hardwareInfo.
    await page.route(/\/api\/hardware$/, async (route, req) => {
      if (req.method() !== 'GET') return route.continue()
      return route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error_code: 'INTERNAL', error: 'hardware probe exploded' }),
      })
    })

    await page.goto(`${baseURL}/settings/hardware`)

    // With no hardware info to fall back on, the page renders the persistent
    // ErrorState (the durable error surface) with a "Try again" action.
    await expect(byTestId(page, 'hardware-settings-error')).toBeVisible({
      timeout: 15000,
    })
    await expect(byTestId(page, 'hardware-settings-error-retry')).toBeVisible()
  })
})
