import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

// audit id all-c72d44950b93 — the hardware error states were never triggered:
// HardwareSettings.tsx fires `message.error("Hardware Error: …")` on a failed
// hardware fetch (lines 56-66) AND, when there is no hardware info to show,
// renders the "Hardware Information Unavailable" error Alert (lines 76-87).
// We force GET /api/hardware to fail so both surfaces appear.
test.describe('Hardware settings error state', () => {
  test('a failed hardware fetch shows the error toast and the unavailable Alert', async ({
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

    // The transient error toast (antd message.error) appears.
    await expect(
      page.locator('.ant-message').getByText(/Hardware Error:/),
    ).toBeVisible({ timeout: 5000 })

    // And, with no hardware info to fall back on, the page renders the
    // "Hardware Information Unavailable" error Alert.
    await expect(
      page.getByText('Hardware Information Unavailable'),
    ).toBeVisible({ timeout: 15000 })
    await expect(page.locator('.ant-alert-error')).toBeVisible()
  })
})
