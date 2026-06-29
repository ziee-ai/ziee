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
import { byTestId } from '../testid'

// recordKey() derives `doi:<lowercased-doi>` for the seeded sampleResult rows.
const KEY1 = 'doi:10.1/aaa'
const KEY2 = 'doi:10.1/bbb'

/**
 * E2E — INDEPENDENT per-row screening decisions via the Segmented control.
 *
 * Audit gap (all-0b649ebd76f3): `per-row-screening.spec.ts` only drives ONE
 * row → Include and asserts a single `Included: 1`. It never proves the
 * per-row Segmented controls are INDEPENDENT — i.e. that distinct rows can
 * hold distinct decisions simultaneously (row A Include + row B Exclude),
 * which is the whole point of a per-record control vs the bulk buttons.
 *
 * This seeds a 2-record result, sets row 1 → Include and row 2 → Exclude via
 * each row's own `<Segmented aria-label="Screening decision">`, and asserts
 * BOTH PRISMA counts (`Included: 1` AND `Excluded: 1`) coexist — a global
 * toggle or a shared-state bug would clobber one of them.
 */

test.describe('Literature — independent per-row Segmented decisions', () => {
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

  test('distinct rows hold distinct decisions (row1 Include + row2 Exclude) simultaneously', async ({
    page,
    testInfra,
  }) => {
    // sampleResult() ships exactly two records.
    await seedLiteratureResult(page, testInfra.baseURL, sampleResult())
    await byTestId(page, 'lit-tool-result-open-button').click()
    await expect(byTestId(page, 'lit-screening-panel')).toBeVisible({ timeout: 10000 })

    // Each record has its own Segmented control (keyed by recordKey).
    await expect(byTestId(page, `lit-screening-record-decision-${KEY1}`)).toBeVisible()
    await expect(byTestId(page, `lit-screening-record-decision-${KEY2}`)).toBeVisible()

    // Row 1 → Include, Row 2 → Exclude, via each row's OWN control.
    await byTestId(page, `lit-screening-record-decision-${KEY1}-opt-include`).click()
    await byTestId(page, `lit-screening-record-decision-${KEY2}-opt-exclude`).click()

    // Both decisions persist independently — neither clobbers the other.
    await expect(byTestId(page, 'lit-screening-tag-included')).toContainText('1', {
      timeout: 10000,
    })
    await expect(byTestId(page, 'lit-screening-tag-excluded')).toContainText('1', {
      timeout: 10000,
    })

    // The two Segmented controls reflect their own (different) selections:
    // the selected ToggleGroup option carries data-state="on".
    await expect(
      byTestId(page, `lit-screening-record-decision-${KEY1}-opt-include`),
    ).toHaveAttribute('data-state', 'on')
    await expect(
      byTestId(page, `lit-screening-record-decision-${KEY2}-opt-exclude`),
    ).toHaveAttribute('data-state', 'on')
  })
})
