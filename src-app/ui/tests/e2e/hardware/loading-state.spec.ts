import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * HardwareSettings renders a Loading spinner ("Loading hardware information...")
 * while the GET /api/hardware request is in flight, before the info cards. This
 * pins that loading branch by HOLDING the hardware fetch open: the spinner must
 * be visible, and once the response is released the Hardware page content
 * replaces it. (Only the HTTP boundary is delayed; the real store→render path
 * runs.)
 */
test.describe('Hardware — loading state', () => {
  test('shows the loading spinner until hardware info resolves', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Gate the hardware-info fetch so the loading branch is observable.
    let release!: () => void
    const gate = new Promise<void>((r) => {
      release = r
    })
    await page.route(/\/api\/hardware$/, async (route) => {
      await gate
      await route.continue()
    })

    await page.goto(`${baseURL}/settings/hardware`)

    // The fetch is held → the loading spinner renders (Loading → Spin role=status).
    await expect(page.getByRole('status').first()).toBeVisible({ timeout: 15000 })

    // Release → the loading branch is replaced by the real page content.
    release()
    await expect(byTestId(page, 'hardware-os-card')).toBeVisible({ timeout: 15000 })
  })
})
