import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'

/**
 * E2E — the "Create Admin Account" submit button shows a loading state
 * while the setup request is in flight (audit gap all-eee9a86cd546).
 *
 * SetupPage.tsx binds the antd Button's `loading` prop to the store's
 * `isSettingUpAdmin` flag (App.store.ts `setupAdmin`), which is true only
 * for the duration of the POST /api/app/setup/admin call. The existing
 * setup.spec.ts asserts the happy-path redirect but never observes this
 * transient loading state.
 *
 * To make the otherwise-instant transition observable we DELAY the real
 * setup response (route.continue after a short wait) — only the timing is
 * altered; the real backend serves the real response, so this is not a
 * cosmetic/mocked-away test.
 */

test.describe('App Setup — submit button loading state', () => {
  test('the Create Admin Account button enters a loading state while setup is in flight', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Hold the real setup request open ~1.2s so the loading spinner is
    // observable, then let it hit the real backend untouched.
    await page.route('**/api/app/setup/admin', async route => {
      await new Promise(resolve => setTimeout(resolve, 1200))
      await route.continue()
    })

    await page.goto(`${baseURL}/setup`)
    await byTestId(page, 'app-setup-username-input').waitFor({ timeout: 30000 })

    await byTestId(page, 'app-setup-username-input').fill('admin')
    await byTestId(page, 'app-setup-email-input').fill('admin@example.com')
    await byTestId(page, 'app-setup-password-input').fill('password123')
    await byTestId(page, 'app-setup-confirm-password-input').fill('password123')

    const submit = byTestId(page, 'app-setup-submit-button')
    await submit.click()

    // While the (delayed) request is in flight the kit Button enters its
    // loading state: aria-busy + activation-blocked (disabled).
    await expect(submit).toHaveAttribute('aria-busy', 'true', {
      timeout: 5000,
    })
    await expect(submit).toBeDisabled()

    // Once the real response lands, setup completes and the app redirects
    // home — proving the loading state cleared on success, not on a fake.
    await expect(page).toHaveURL(`${baseURL}/`, { timeout: 15000 })
  })
})
