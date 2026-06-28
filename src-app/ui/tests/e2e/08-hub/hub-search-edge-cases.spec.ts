import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import { searchHubResources, clearAllFilters } from './helpers/hub-search-filter'
import { getModelCards } from './helpers/hub-models'

/**
 * E2E — hub search special-character / edge-case robustness (audit bf76401abe79).
 *
 * The existing `05-hub-search-filter.spec.ts` only drives plain alphabetic
 * terms ("llama", "code", "file"). The hub search is a literal
 * `name/title/description.toLowerCase().includes(query.toLowerCase())` filter
 * (e.g. McpServersHubTab.tsx:53-61, AssistantsHubTab.tsx:40-46) — so the
 * security/robustness contract is that a regex-special query is treated as a
 * LITERAL substring (never compiled as a regex), an unmatched query shows the
 * empty state without throwing, and clearing restores the full catalog. None of
 * those edge cases were exercised. This drives them against the real (seeded)
 * models catalog.
 */
test.describe('Hub Search — special characters and edge cases', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('regex-special and edge-case queries filter literally without crashing', async ({
    page,
    testInfra,
  }) => {
    await navigateToHub(page, testInfra.baseURL, 'models')
    await waitForHubDataLoad(page)

    const initialCount = await (await getModelCards(page)).count()
    expect(initialCount).toBeGreaterThan(0)

    // (a) ".*" — as a regex this matches EVERYTHING; as the literal substring
    // the code actually uses, it matches only items containing the chars ".*"
    // (none in the seeded catalog). A count of 0 proves the query is NOT
    // compiled as a regex (a regex bug would return `initialCount`).
    await searchHubResources(page, '.*')
    expect(await (await getModelCards(page)).count()).toBe(0)
    await expect(
      page.getByText(/no models match|no.*results|no.*found/i),
    ).toBeVisible()

    // (b) An invalid-regex string (unbalanced bracket). If this were ever fed to
    // `new RegExp(...)` it would throw and break the list; as a literal
    // substring it simply matches nothing and renders the empty state.
    await searchHubResources(page, 'a.*[b(c+')
    expect(await (await getModelCards(page)).count()).toBe(0)
    await expect(
      page.getByText(/no models match|no.*results|no.*found/i),
    ).toBeVisible()

    // (c) Unicode + a very long no-match query — must stay robust (no crash,
    // empty state), not error out.
    await searchHubResources(page, 'zürich—🤖' + 'x'.repeat(500))
    expect(await (await getModelCards(page)).count()).toBe(0)
    await expect(
      page.getByText(/no models match|no.*results|no.*found/i),
    ).toBeVisible()

    // (d) Clearing the search restores the full catalog (empty query => no
    // filter applied), proving the edge-case queries left no sticky state.
    await clearAllFilters(page)
    expect(await (await getModelCards(page)).count()).toBe(initialCount)
  })
})
