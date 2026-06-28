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
test.describe('Literature inline card — zero results', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(() => JSON.parse(localStorage.getItem('auth-storage')!).state.token)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('renders the empty-results state for a query with no records', async ({ page, testInfra }) => {
    await seedLiteratureResult(page, testInfra.baseURL, {
      query: 'an obscure query with no hits',
      records: [],
      identified: {},
      after_dedup: 0,
      degraded_sources: [],
      completeness: null,
    })
    await expect(page.getByText('No records returned')).toBeVisible({ timeout: 15000 })
  })
})
