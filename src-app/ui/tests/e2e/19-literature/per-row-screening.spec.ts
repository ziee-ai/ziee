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
 * E2E — per-row screening Segmented decisions + the preprint badge in the
 * literature screening panel.
 *
 * Audit gaps:
 *   - screening-flow.spec only drives the BULK include/exclude buttons; the
 *     per-row `<Segmented aria-label="Screening decision">` control was untested.
 *   - the `{r.is_preprint && <Tag>preprint</Tag>}` badge was never asserted.
 */

test.describe('Literature — per-row screening', () => {
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

  test('clicking a per-row Include segment updates the PRISMA count', async ({
    page,
    testInfra,
  }) => {
    await seedLiteratureResult(page, testInfra.baseURL, sampleResult())
    await page.getByRole('button', { name: /Open in screening/ }).click()
    await expect(
      page.getByRole('heading', { name: 'Screening' }),
    ).toBeVisible({ timeout: 10000 })

    // The first record's own Segmented control → Include.
    await page
      .locator('[aria-label="Screening decision"]')
      .first()
      .getByText('Include', { exact: true })
      .click()

    await expect(page.getByText('Included: 1')).toBeVisible({ timeout: 10000 })
  })

  test('a preprint record renders the "preprint" badge', async ({
    page,
    testInfra,
  }) => {
    const result = sampleResult()
    result.records[0].is_preprint = true
    await seedLiteratureResult(page, testInfra.baseURL, result)

    await page.getByRole('button', { name: /Open in screening/ }).click()
    await expect(
      page.getByRole('heading', { name: 'Screening' }),
    ).toBeVisible({ timeout: 10000 })

    await expect(page.getByText('preprint', { exact: true })).toBeVisible({
      timeout: 10000,
    })
  })
})
