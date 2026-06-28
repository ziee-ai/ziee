import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToRepositoriesPage,
  waitForRepositoriesPageLoad,
  openAddRepositoryDrawer,
} from './helpers/repository-helpers'

// Coverage for previously-untested LlmRepositoryDrawer behaviors:
//   - fa516d97e5f1 — built-in repository auth-field editing through the UI
//   - 8dc42d659a43 — Test Connection button conditional visibility (partial auth)
//   - 13a18f59293f — Drawer mask is NOT closable (mask:{closable:false})

test.describe('LLM Repository drawer — extras', () => {
  test.describe.configure({ retries: 2 })

  // 8dc42d659a43 — showTestButton requires url AND the auth field for the
  // selected auth_type. With API-Key auth, the Test Connection button must stay
  // hidden until BOTH the URL and the api_key are filled.
  test('Test Connection button appears only once URL + api_key are both filled', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await goToRepositoriesPage(page, testInfra.baseURL)
    await waitForRepositoriesPageLoad(page)
    await openAddRepositoryDrawer(page)

    const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
    const testButton = drawer.locator('button:has-text("Test Connection")')

    // Pick API Key auth + a URL but leave the api_key empty → button hidden.
    await page.fill('#llm-repository-form_name', 'Partial Auth Repo')
    await page.fill('#llm-repository-form_url', 'https://example.com/api')
    await page.click('.ant-select:has(#llm-repository-form_auth_type)')
    await page.waitForSelector('.ant-select-dropdown', { state: 'visible' })
    await page.click('.ant-select-item-option:has-text("API Key")')

    await expect(testButton).toHaveCount(0)

    // Provide the api_key → the button becomes visible.
    await page.fill('#llm-repository-form_api_key', 'sk-partial-auth-123')
    await expect(testButton).toBeVisible({ timeout: 10000 })

    await drawer.locator('button:has-text("Cancel")').click()
  })

  // 13a18f59293f — the drawer is opened with mask:{closable:false}; clicking the
  // mask overlay must NOT close it (prevents losing in-progress edits).
  test('clicking the drawer mask does not close the drawer', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await goToRepositoriesPage(page, testInfra.baseURL)
    await waitForRepositoriesPageLoad(page)
    await openAddRepositoryDrawer(page)

    const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
    await expect(drawer).toBeVisible()

    // Click the mask overlay (force: it sits under the drawer panel z-stack).
    await page.locator('.ant-drawer-mask').click({ force: true, position: { x: 5, y: 5 } })

    // The drawer is still open — the mask is non-closable.
    await expect(
      page.locator('.ant-drawer-title:has-text("Add Repository")'),
    ).toBeVisible()

    await drawer.locator('button:has-text("Cancel")').click()
  })

  // fa516d97e5f1 — a built-in repository's name/url are locked, but its auth
  // fields are editable and saving the auth change round-trips through the UI.
  test('built-in repository auth fields are editable and save', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await goToRepositoriesPage(page, testInfra.baseURL)
    await waitForRepositoriesPageLoad(page)

    const hfRepo = page
      .locator('div')
      .filter({ hasText: /^Hugging Face/ })
      .first()
    await expect(hfRepo).toBeVisible({ timeout: 15000 })
    await hfRepo.locator('button:has-text("Edit")').click()
    await page.waitForSelector('.ant-drawer-title', { timeout: 30000 })

    const drawer = page.locator('.ant-drawer.ant-drawer-open').last()

    // Built-in: identity is locked …
    await expect(page.locator('#llm-repository-form_name')).toBeDisabled()
    await expect(page.locator('#llm-repository-form_url')).toBeDisabled()

    // … but the API-key auth field is editable.
    const apiKeyField = page.locator('#llm-repository-form_api_key')
    await expect(apiKeyField).toBeEnabled()
    await apiKeyField.fill('hf_builtin_auth_edit_token')

    await drawer.locator('.ant-btn-primary[type="submit"]').click()

    // The auth-only update round-trips (no enabled-transition probe).
    await expect(
      page.getByText('Repository updated successfully'),
    ).toBeVisible({ timeout: 15000 })
  })
})
