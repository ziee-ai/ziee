import { test, expect } from '../../fixtures/test-context'

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
    await page.getByLabel('Username').waitFor({ timeout: 30000 })
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Email').fill('admin@example.com')
    await page.getByLabel('Password', { exact: true }).fill('password123')
    await page.getByLabel('Confirm Password').fill('password123')

    const submit = page.getByRole('button', { name: /create admin account/i })
    await submit.click()

    // While the delayed request is in flight, the antd Button renders its
    // loading spinner (class `ant-btn-loading`).
    await expect(submit).toHaveClass(/ant-btn-loading/, { timeout: 5000 })

    // The request eventually completes and the wizard navigates away from /setup.
    await expect(page).not.toHaveURL(/\/setup/, { timeout: 20000 })
  })
})
