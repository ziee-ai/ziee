import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  seedLiteratureResult,
  type LitStructured,
} from './fixtures/mock-literature-result'

// audit id 5cfdc68cb868 — the inline LiteratureToolResultCard zero-results
// branch (records.length === 0, LiteratureToolResultCard.tsx:67-73) was never
// E2E-tested. Seed a `literature_search` tool_result whose structured_content
// has an empty `records` array and assert the card renders the "No records"
// message AND hides the "Open in screening" affordance.

function emptyResult(degraded: string[]): LitStructured {
  return {
    query: 'an exceedingly obscure query with no hits',
    records: [],
    identified: {},
    after_dedup: 0,
    degraded_sources: degraded,
    completeness: null,
  }
}

test.describe('Literature inline card — zero results', () => {
  test.describe.configure({ retries: 2 })

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(
      () => JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('no records → "for this query" message + no screening button', async ({
    page,
    testInfra,
  }) => {
    await seedLiteratureResult(page, testInfra.baseURL, emptyResult([]))

    await expect(page.getByText(/No records returned/i)).toBeVisible({
      timeout: 10000,
    })
    await expect(page.getByText(/for this query/i)).toBeVisible()
    // The "Open in screening" CTA only renders when there are records.
    await expect(
      page.getByRole('button', { name: /Open in screening/ }),
    ).toHaveCount(0)
  })

  test('no records due to degraded sources → "every source errored" note', async ({
    page,
    testInfra,
  }) => {
    await seedLiteratureResult(
      page,
      testInfra.baseURL,
      emptyResult(['europepmc', 'crossref']),
    )

    await expect(page.getByText(/No records returned/i)).toBeVisible({
      timeout: 10000,
    })
    await expect(
      page.getByText(/every source errored or was skipped/i),
    ).toBeVisible()
    // The degraded-sources warning lists the skipped engines.
    await expect(page.getByText(/2 sources degraded\/skipped/i)).toBeVisible()
    await expect(
      page.getByRole('button', { name: /Open in screening/ }),
    ).toHaveCount(0)
  })
})
