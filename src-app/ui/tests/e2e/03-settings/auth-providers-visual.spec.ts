/**
 * Visual smoke screenshots for the auth-providers admin UI. Not run
 * by default in CI — invoke explicitly to refresh artifacts I can
 * eyeball when there's no human in the loop:
 *
 *   npx playwright test tests/e2e/03-settings/auth-providers-visual.spec.ts
 *
 * Drops PNGs into test-results/<id>/.../*.png that pass through the
 * normal Playwright artifact pipeline.
 */
import { test } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

test.describe('Auth providers — visual smoke', () => {
  test('list page + edit drawer + delete popconfirm', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    // 1. List with the three pre-seeded disabled rows.
    await page.waitForLoadState('networkidle')
    await page.screenshot({
      path: 'test-results/visual-list-empty.png',
      fullPage: true,
    })

    // 2. Open Add menu (template dropdown).
    await page.getByRole('button', { name: /add provider/i }).click()
    await page.waitForTimeout(300) // let dropdown animation settle
    await page.screenshot({
      path: 'test-results/visual-add-menu.png',
      fullPage: true,
    })

    // 3. Click Generic OIDC → drawer opens with the form.
    await page.getByText(/Generic OIDC \(Auth0/i).click()
    await page.waitForLoadState('networkidle')
    await page.screenshot({
      path: 'test-results/visual-edit-drawer-empty.png',
      fullPage: true,
    })

    // 4. Drawer with fields filled (pre-create state).
    await page.getByLabel(/Name \(URL slug\)/i).fill('visual-test')
    await page.getByLabel(/Client ID/i).fill('my-client-id')
    await page.locator('input[type="password"]').first().fill('my-secret')
    await page.getByLabel(/Issuer URL/i).fill('https://example.invalid/oidc')
    await page.screenshot({
      path: 'test-results/visual-edit-drawer-filled.png',
      fullPage: true,
    })

    // 5. Click Test config → shows inline result alert.
    await page.getByRole('button', { name: /test config/i }).click()
    await page.waitForTimeout(2000)
    await page.screenshot({
      path: 'test-results/visual-edit-drawer-tested.png',
      fullPage: true,
    })

    // 6. Cancel back to the list.
    await page.getByRole('button', { name: /^Cancel$/ }).click()
    await page.waitForTimeout(300)

    // 7. Open delete popconfirm for the apple row (was a Modal in
    // an earlier revision; now an inline Popconfirm).
    const appleRow = page.getByRole('row').filter({ hasText: 'apple' }).first()
    await appleRow.getByRole('button', { name: /^Delete apple$/i }).click()
    await page.waitForTimeout(300)
    await page.screenshot({
      path: 'test-results/visual-delete-popconfirm.png',
      fullPage: true,
    })
  })
})
