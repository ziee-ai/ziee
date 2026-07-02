import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToRepositoriesPage,
  waitForRepositoriesPageLoad,
  openAddRepositoryDrawer,
} from './helpers/repository-helpers'
import { byTestId } from '../testid'

// Coverage for previously-untested LlmRepositoryDrawer behaviors:
//   - fa516d97e5f1 — built-in repository auth-field editing through the UI
//   - 8dc42d659a43 — Test Connection button conditional visibility (partial auth)
//   - 13a18f59293f — Drawer mask is NOT closable (maskClosable:false)

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

    const testButton = byTestId(page, 'llmrepo-form-test-btn')

    // Pick API Key auth + a URL but leave the api_key empty → button hidden.
    await byTestId(page, 'llmrepo-form-name').fill('Partial Auth Repo')
    await byTestId(page, 'llmrepo-form-url').fill('https://example.com/api')
    await byTestId(page, 'llmrepo-form-auth-type').click()
    await byTestId(page, 'llmrepo-form-auth-type-opt-api_key').click()

    await expect(testButton).toHaveCount(0)

    // Provide the api_key → the button becomes visible.
    await byTestId(page, 'llmrepo-form-api-key').fill('sk-partial-auth-123')
    await expect(testButton).toBeVisible({ timeout: 10000 })

    await byTestId(page, 'llmrepo-form-cancel-btn').click()
  })

  // 13a18f59293f — the drawer is opened with maskClosable:false; clicking
  // outside the panel must NOT close it (prevents losing in-progress edits).
  test('clicking the drawer mask does not close the drawer', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await goToRepositoriesPage(page, testInfra.baseURL)
    await waitForRepositoriesPageLoad(page)
    await openAddRepositoryDrawer(page)

    await expect(byTestId(page, 'llmrepo-form')).toBeVisible()

    // Click the overlay area (top-left corner, outside the right-side panel).
    await page.mouse.click(5, 5)

    // The drawer is still open — the mask is non-closable.
    await expect(byTestId(page, 'llmrepo-form')).toBeVisible()

    await byTestId(page, 'llmrepo-form-cancel-btn').click()
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
      .locator('[data-testid^="llmrepo-row-"]')
      .filter({ hasText: 'Hugging Face' })
      .first()
    await expect(hfRepo).toBeVisible({ timeout: 15000 })
    await hfRepo.locator('[data-testid^="llmrepo-edit-btn-"]').first().click()
    await byTestId(page, 'llmrepo-form').waitFor({ timeout: 30000 })

    // Built-in: identity is locked …
    await expect(byTestId(page, 'llmrepo-form-name')).toBeDisabled()
    await expect(byTestId(page, 'llmrepo-form-url')).toBeDisabled()

    // … but the API-key auth field is editable.
    const apiKeyField = byTestId(page, 'llmrepo-form-api-key')
    await expect(apiKeyField).toBeEnabled()
    await apiKeyField.fill('hf_builtin_auth_edit_token')

    // The auth-only update round-trips (no enabled-transition probe).
    const [resp] = await Promise.all([
      page.waitForResponse(
        r => /\/api\/llm-repositories\/[0-9a-f-]+/.test(r.url()) && r.request().method() === 'POST',
        { timeout: 15000 }
      ),
      byTestId(page, 'llmrepo-form-submit-btn').click(),
    ])
    expect(resp.ok()).toBeTruthy()
  })
})
