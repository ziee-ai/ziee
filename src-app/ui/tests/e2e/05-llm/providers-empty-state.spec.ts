import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToProvidersPage,
  waitForProvidersPageLoad,
} from './helpers/navigation-helpers'
import { createLocalProvider } from './helpers/provider-helpers'
import { byTestId } from '../testid'

/**
 * E2E — the zero-providers empty state on the LLM Providers settings page
 * (audit gap all-f2e47017e178).
 *
 * The existing `LLM Providers - Empty States` test only asserts the "Add
 * Provider" menu item exists; it never deletes all providers and verifies
 * what the page renders with ZERO providers. This test drives the real
 * transition: it ensures at least one provider exists (created through the
 * UI), then deletes EVERY provider via the real REST API, reloads, and
 * asserts the empty state — the provider sidebar collapses to just the
 * "Add Provider" item and the main pane shows the "No provider selected"
 * Empty (LlmProviderSettings.tsx:124-129). Only the teardown delete uses
 * the API (the setup boundary); the assertion is on the real rendered UI.
 */

async function tokenFromPage(page: import('@playwright/test').Page): Promise<string> {
  return page.evaluate(
    () => JSON.parse(localStorage.getItem('auth-storage')!).state.token,
  )
}

async function deleteAllProviders(
  page: import('@playwright/test').Page,
  apiURL: string,
  token: string,
): Promise<void> {
  const listRes = await page.request.get(`${apiURL}/api/llm-providers`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  expect(listRes.ok(), `list providers: ${listRes.status()}`).toBeTruthy()
  const body = await listRes.json()
  const providers: Array<{ id: string }> = body.providers ?? []
  for (const p of providers) {
    const del = await page.request.delete(
      `${apiURL}/api/llm-providers/${p.id}`,
      { headers: { Authorization: `Bearer ${token}` } },
    )
    expect(del.ok(), `delete ${p.id}: ${del.status()}`).toBeTruthy()
  }
}

test.describe('LLM Providers - Empty State (all deleted)', () => {
  test('deleting every provider renders the empty state, Add Provider still available', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Ensure ≥1 provider exists so the deletion is a real transition, and
    // confirm it renders as a provider menu item in the sidebar.
    const providerName = `EmptyState_${Date.now().toString(36)}`
    await createLocalProvider(page, baseURL, providerName, 'empty-state probe')
    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)
    await expect(
      page.locator('[data-testid^="llm-provider-nav-"]').filter({ hasText: providerName }),
    ).toBeVisible()

    // Delete EVERY provider via the real API, then reload the page.
    const token = await tokenFromPage(page)
    await deleteAllProviders(page, apiURL, token)
    await goToProvidersPage(page, baseURL)
    await waitForProvidersPageLoad(page)

    // Empty state: no provider rows remain in the sidebar, but the
    // "Add Provider" affordance is still present (the only nav button left)...
    await expect(
      page.locator('[data-testid^="llm-provider-nav-"]').filter({ hasText: providerName }),
    ).toHaveCount(0)
    const navButtons = page.locator('[data-testid^="llm-provider-nav-"]')
    await expect(navButtons).toHaveCount(1)
    await expect(byTestId(page, 'llm-provider-nav-add-provider')).toBeVisible()

    // ...and the main pane shows the "No provider selected" Empty.
    await expect(byTestId(page, 'llm-provider-settings-empty')).toBeVisible()
  })
})
