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
    await page.getByRole('button', { name: /Open in screening/ }).click()
    await expect(
      page.getByRole('heading', { name: 'Screening' }),
    ).toBeVisible({ timeout: 10000 })

    const segments = page.locator('[aria-label="Screening decision"]')
    await expect(segments).toHaveCount(2)

    // Row 1 → Include, Row 2 → Exclude, via each row's OWN control.
    await segments.nth(0).getByText('Include', { exact: true }).click()
    await segments.nth(1).getByText('Exclude', { exact: true }).click()

    // Both decisions persist independently — neither clobbers the other.
    await expect(page.getByText('Included: 1')).toBeVisible({ timeout: 10000 })
    await expect(page.getByText('Excluded: 1')).toBeVisible({ timeout: 10000 })

    // The two Segmented controls reflect their own (different) selections:
    // antd marks the selected segment label with `.ant-segmented-item-selected`.
    await expect(
      segments.nth(0).locator('.ant-segmented-item-selected'),
    ).toHaveText('Include')
    await expect(
      segments.nth(1).locator('.ant-segmented-item-selected'),
    ).toHaveText('Exclude')
  })
})
