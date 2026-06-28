import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

// audit id all-b42fa45789cb — the hardware page's loading state
// (HardwareSettings.tsx:68-74: a "Loading hardware information..." spinner while
// `hardwareLoading` is true) was never verified. We delay the GET /api/hardware
// response so the transient loading branch is observable, assert the spinner
// tip renders, then assert it is replaced by real content once the (mocked)
// response resolves.
test.describe('Hardware settings loading state', () => {
  test('shows the loading spinner while hardware info is in flight, then content', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Hold the hardware-info response for ~1.5s so the loading branch is
    // reliably visible, then return a minimal valid payload.
    await page.route(/\/api\/hardware$/, async (route, req) => {
      if (req.method() !== 'GET') return route.continue()
      await new Promise((r) => setTimeout(r, 1500))
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          hardware: {
            os_name: 'Linux',
            os_version: 'test',
            cpu_brand: 'Test CPU',
            cpu_cores: 8,
            total_memory: 16 * 1024 * 1024 * 1024,
            gpus: [],
          },
        }),
      })
    })

    await page.goto(`${baseURL}/settings/hardware`)

    // The loading tip must appear while the request is in flight.
    await expect(
      page.getByText('Loading hardware information...'),
    ).toBeVisible({ timeout: 5000 })

    // Once the response resolves, the spinner is replaced by real content.
    await expect(
      page.getByText('Loading hardware information...'),
    ).toHaveCount(0, { timeout: 15000 })
    await expect(page.getByText('Operating System')).toBeVisible({
      timeout: 15000,
    })
  })
})
