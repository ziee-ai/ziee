import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'

/**
 * E2E — SetupPage "Create Admin Account" button loading state.
 *
 * The submit button is `loading={isSettingUpAdmin}` (SetupPage.tsx:163-172),
 * driven by App.store while the POST /api/app/setup/admin is in flight. The
 * setup request is delayed (then passed through to the real backend) so the
 * transient loading state is observable.
 */

test.describe('App Setup — submit loading state', () => {
  test('the create-admin button shows a loading spinner while submitting', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Delay the setup POST so the in-flight loading state is observable, then
    // pass through to the real backend (do NOT fake the response).
    await page.route('**/api/app/setup/admin', async route => {
      await new Promise(r => setTimeout(r, 1500))
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

    // While the delayed request is in flight, the kit Button enters its loading
    // state (aria-busy).
    await expect(submit).toHaveAttribute('aria-busy', 'true', { timeout: 5000 })

    // The request eventually completes and the wizard navigates away from /setup.
    await expect(page).not.toHaveURL(/\/setup/, { timeout: 20000 })
  })
})
