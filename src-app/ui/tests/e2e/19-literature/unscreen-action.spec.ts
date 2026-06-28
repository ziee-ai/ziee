import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedLiteratureResult, sampleResult } from './fixtures/mock-literature-result'

// The bulk "Unscreen" action (LiteratureScreeningPanel.tsx:175-177) calls
// bulkDecide('unscreened') on the checkbox selection — resetting prior
// Include/Exclude decisions back to unscreened. screening-flow.spec.ts only
// covers Include/Exclude; this proves Unscreen actually clears decisions and
// the PRISMA counts return to zero.

test.describe('Literature screening — Unscreen bulk action', () => {
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

  test('Unscreen resets bulk decisions back to unscreened (counts → 0)', async ({
    page,
    testInfra,
  }) => {
    await seedLiteratureResult(page, testInfra.baseURL, sampleResult())

    await page.getByRole('button', { name: /Open in screening/ }).click()
    await expect(page.getByRole('heading', { name: 'Screening' })).toBeVisible({
      timeout: 10000,
    })

    // Bulk-include both rows → Included: 2 (non-zero precondition).
    await page.getByRole('checkbox', { name: /Select all|selected/ }).click()
    await page.getByRole('button', { name: 'Include', exact: true }).click()
    await expect(page.getByText('Included: 2')).toBeVisible()

    // Each row's Segmented now reflects Include.
    const segmenteds = page.getByLabel('Screening decision')
    await expect(segmenteds.first().locator('.ant-segmented-item-selected')).toHaveText(
      'Include',
    )

    // Re-select all and click Unscreen → decisions cleared, counts reset.
    await page.getByRole('checkbox', { name: /Select all|selected/ }).click()
    await page.getByRole('button', { name: 'Unscreen', exact: true }).click()

    await expect(page.getByText('Included: 0')).toBeVisible()
    await expect(page.getByText('Excluded: 0')).toBeVisible()

    // Both rows' Segmented controls are back to Unscreened.
    await expect(
      segmenteds.first().locator('.ant-segmented-item-selected'),
    ).toHaveText('Unscreened')
    await expect(
      segmenteds.last().locator('.ant-segmented-item-selected'),
    ).toHaveText('Unscreened')
  })
})
