import { readFileSync } from 'fs'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  seedLiteratureResult,
  sampleResult,
} from './fixtures/mock-literature-result'

/**
 * Complete research-screening journey through the UI: a seeded literature
 * search result → open the screening workbench → read the PRISMA identified /
 * dedup counts + the saturation (completeness) estimate → include the records →
 * export a citation file (RIS) carrying the included studies. The underlying
 * research TOOLS (literature_search aggregation, fetch_paper_fulltext,
 * select_included, citations) are covered by backend integration tests; this
 * is the UI-centric end-to-end of the journey (deterministic, no live LLM).
 */
test.describe('Literature research journey', () => {
  test.describe.configure({ retries: 2 })

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

  test('search result → PRISMA + saturation → include → export RIS', async ({
    page,
    testInfra,
  }) => {
    await seedLiteratureResult(page, testInfra.baseURL, sampleResult())

    // Open the screening workbench from the inline result card.
    await page.getByRole('button', { name: /Open in screening/ }).click()
    await expect(page.getByRole('heading', { name: 'Screening' })).toBeVisible({
      timeout: 10000,
    })

    // PRISMA provenance: dedup count + the saturation (completeness) estimate.
    await expect(page.getByText('After dedup: 2')).toBeVisible()
    await expect(page.getByText(/Saturation estimate: MODERATE/i)).toBeVisible()

    // Include every record → PRISMA Included reflects it.
    await page.getByRole('checkbox', { name: /Select all|selected/ }).click()
    await page.getByRole('button', { name: 'Include', exact: true }).click()
    await expect(page.getByText('Included: 2')).toBeVisible()

    // Export the included studies as RIS (the "cite" leg of the journey).
    await page.getByRole('button', { name: /Export (all|included)/ }).click()
    const download = page.waitForEvent('download')
    await page.getByRole('menuitem', { name: 'Export RIS' }).click()
    const file = await download
    expect(file.suggestedFilename()).toBe('screening.ris')

    const ris = readFileSync(await file.path(), 'utf8')
    // RIS records start with a type tag and carry the included study title.
    expect(ris).toContain('TY  -')
    expect(ris).toContain('Base editing reduces off-target effects')
  })
})
