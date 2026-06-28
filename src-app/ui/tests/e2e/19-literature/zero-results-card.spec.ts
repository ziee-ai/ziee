import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedLiteratureResult } from './fixtures/mock-literature-result'

// audit id all-bb15fde043d4 — the literature inline card's zero-results branch
// (LiteratureToolResultCard.tsx:67-73) was untested. Seed a literature_search
// tool_result with EMPTY records and assert the "No records returned" state.
import { seedLiteratureResult, type LitStructured } from './fixtures/mock-literature-result'

// Deterministic zero-results coverage for LiteratureToolResultCard
// (LiteratureToolResultCard.tsx:67-73). When a `literature_search` tool_result
// carries an EMPTY records array, the inline card must render its zero-results
// empty state ("No records returned … for this query.") and NOT the records
// list / "Open in screening" button. The existing screening-flow spec only
// seeds a populated result, so this branch was untested.

test.describe('Literature inline card — zero results', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(() => JSON.parse(localStorage.getItem('auth-storage')!).state.token)
    const token = await page.evaluate(() =>
      JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('renders the empty-results state for a query with no records', async ({ page, testInfra }) => {
    await seedLiteratureResult(page, testInfra.baseURL, {
      query: 'an obscure query with no hits',
  test('an empty literature_search result renders the no-records empty state', async ({
    page,
    testInfra,
  }) => {
    const empty: LitStructured = {
      query: 'a query that matches nothing',
      records: [],
      identified: {},
      after_dedup: 0,
      degraded_sources: [],
      completeness: null,
    })
    await expect(page.getByText('No records returned')).toBeVisible({ timeout: 15000 })
    }
    await seedLiteratureResult(page, testInfra.baseURL, empty)

    // The inline card itself renders (header + zero-results body).
    await expect(page.getByText('Literature search').first()).toBeVisible({ timeout: 10000 })
    // The zero-results branch (records.length === 0, no degraded sources).
    await expect(page.getByText(/No records returned for this query\./)).toBeVisible()

    // The records-list path must NOT render: no "Open in screening" button.
    await expect(
      page.getByRole('button', { name: /Open in screening/ }),
    ).toHaveCount(0)
  })
})
