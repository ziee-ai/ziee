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
import { byTestId } from '../testid.ts'

test.describe('Auth providers — visual smoke', () => {
  test('list page + edit drawer + delete confirm', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/auth-providers`)

    // 1. List with the three pre-seeded disabled rows.
    await page.waitForLoadState('load')
    await page.screenshot({
      path: 'test-results/visual-list-empty.png',
      fullPage: true,
    })

    // 2. Open Add menu (template dropdown — a `+` icon button).
    await byTestId(page, 'authprov-add-button').click()
    await page.waitForTimeout(300) // let dropdown animation settle
    await page.screenshot({
      path: 'test-results/visual-add-menu.png',
      fullPage: true,
    })

    // 3. Click Generic OIDC → drawer opens with the form.
    await byTestId(page, 'authprov-add-dropdown-item-oidc-generic').click()
    await page.waitForLoadState('load')
    await page.screenshot({
      path: 'test-results/visual-edit-drawer-empty.png',
      fullPage: true,
    })

    // 4. Drawer with fields filled (pre-create state).
    await byTestId(page, 'authprov-name-input').fill('visual-test')
    await byTestId(page, 'authprov-oidc-client-id-input').fill('my-client-id')
    await byTestId(page, 'authprov-oidc-client-secret-input').fill('my-secret')
    await byTestId(page, 'authprov-oidc-issuer-url-input').fill(
      'https://example.invalid/oidc',
    )
    await page.screenshot({
      path: 'test-results/visual-edit-drawer-filled.png',
      fullPage: true,
    })

    // 5. Click Test config → shows inline result alert.
    await byTestId(page, 'authprov-test-config-button').click()
    await page.waitForTimeout(2000)
    await page.screenshot({
      path: 'test-results/visual-edit-drawer-tested.png',
      fullPage: true,
    })

    // 6. Cancel back to the list.
    await byTestId(page, 'authprov-drawer-cancel-button').click()
    await page.waitForTimeout(300)

    // 7. Open the delete confirm for the apple row (inline confirm dialog).
    await byTestId(page, 'authprov-delete-button-apple').click()
    await page.waitForTimeout(300)
    await page.screenshot({
      path: 'test-results/visual-delete-popconfirm.png',
      fullPage: true,
    })
  })
})
