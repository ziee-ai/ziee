import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { openAddLocalProvider, submitOpenDrawer } from './helpers/local-runtime-helpers'

/**
 * Local provider create flow: when type=local the form hides base_url +
 * api_key and, on create, surfaces the one-time proxy token.
 */
test.describe('Local Runtime — local provider create', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('local type hides base_url and api_key fields', async ({ page, testInfra }) => {
    await openAddLocalProvider(page, testInfra.baseURL)
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    // No API key / base URL inputs for local providers.
    await expect(drawer.getByLabel(/API Key/i)).toHaveCount(0)
    await expect(drawer.getByLabel(/Base URL/i)).toHaveCount(0)
    // An info note explains local providers need no API key.
    await expect(
      drawer.getByText(/Local providers don't require API keys/i)
    ).toBeVisible()
  })

  test('creating a local provider surfaces the one-time token', async ({ page, testInfra }) => {
    await openAddLocalProvider(page, testInfra.baseURL)
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await drawer.locator('input').first().fill(`local-${Date.now()}`)
    await submitOpenDrawer(page)
    // One-time token banner / copyable secret appears after create.
    await expect(page.getByText(/API key|token/i).first()).toBeVisible({ timeout: 10000 })
  })
})
