import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'
import { openAddLocalProvider, submitOpenDrawer } from './helpers/local-runtime-helpers'

/**
 * Local provider create flow: when type=local the form hides base_url +
 * api_key and shows the local-provider note; create succeeds.
 */
test.describe('Local Runtime — local provider create', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('local type hides base_url and api_key fields', async ({ page, testInfra }) => {
    await openAddLocalProvider(page, testInfra.baseURL)
    const drawer = byTestId(page, 'llm-provider-form')
    // No API key / base URL inputs for local providers.
    await expect(byTestId(drawer, 'llm-provider-api-key-input')).toHaveCount(0)
    await expect(byTestId(drawer, 'llm-provider-base-url-input')).toHaveCount(0)
    // An info note explains local providers need no API key.
    await expect(byTestId(drawer, 'llm-provider-local-note')).toBeVisible()
  })

  test('creating a local provider succeeds', async ({ page, testInfra }) => {
    await openAddLocalProvider(page, testInfra.baseURL)
    await byTestId(page, 'llm-provider-name-input').fill(`local-${Date.now()}`)
    await submitOpenDrawer(page)
    // On success the create form closes and a confirmation toast appears.
    await expect(page.locator('[data-sonner-toast]').first()).toBeVisible({ timeout: 10000 })
    await expect(byTestId(page, 'llm-provider-form')).toHaveCount(0)
  })
})
