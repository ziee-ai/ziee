import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedLiteratureResult, type LitStructured } from './fixtures/mock-literature-result'
import { byTestId } from '../testid'

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
    const token = await page.evaluate(() =>
      JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

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
    }
    await seedLiteratureResult(page, testInfra.baseURL, empty)

    // The inline card itself renders (header + zero-results body).
    await expect(byTestId(page, 'lit-tool-result-card')).toBeVisible({ timeout: 10000 })
    // The zero-results branch (records.length === 0, no degraded sources).
    await expect(byTestId(page, 'lit-tool-result-empty')).toBeVisible()
    await expect(byTestId(page, 'lit-tool-result-empty')).toContainText('for this query')

    // The records-list path must NOT render: no "Open in screening" button.
    await expect(byTestId(page, 'lit-tool-result-open-button')).toHaveCount(0)
  })
})
