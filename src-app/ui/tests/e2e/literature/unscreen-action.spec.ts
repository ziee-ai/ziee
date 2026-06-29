import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedLiteratureResult, sampleResult } from './fixtures/mock-literature-result'
import { byTestId } from '../testid'

// recordKey() derives `doi:<lowercased-doi>` for the seeded sampleResult rows.
const KEY1 = 'doi:10.1/aaa'
const KEY2 = 'doi:10.1/bbb'

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

    await byTestId(page, 'lit-tool-result-open-button').click()
    await expect(byTestId(page, 'lit-screening-panel')).toBeVisible({
      timeout: 10000,
    })

    // Bulk-include both rows → Included: 2 (non-zero precondition).
    await byTestId(page, 'lit-screening-select-all-checkbox').click()
    await byTestId(page, 'lit-screening-bulk-include-button').click()
    await expect(byTestId(page, 'lit-screening-tag-included')).toContainText('2')

    // Each row's Segmented now reflects Include (selected option data-state="on").
    await expect(
      byTestId(page, `lit-screening-record-decision-${KEY1}-opt-include`),
    ).toHaveAttribute('data-state', 'on')

    // Re-select all and click Unscreen → decisions cleared, counts reset.
    await byTestId(page, 'lit-screening-select-all-checkbox').click()
    await byTestId(page, 'lit-screening-bulk-unscreen-button').click()

    await expect(byTestId(page, 'lit-screening-tag-included')).toContainText('0')
    await expect(byTestId(page, 'lit-screening-tag-excluded')).toContainText('0')

    // Both rows' Segmented controls are back to Unscreened.
    await expect(
      byTestId(page, `lit-screening-record-decision-${KEY1}-opt-unscreened`),
    ).toHaveAttribute('data-state', 'on')
    await expect(
      byTestId(page, `lit-screening-record-decision-${KEY2}-opt-unscreened`),
    ).toHaveAttribute('data-state', 'on')
  })
})
