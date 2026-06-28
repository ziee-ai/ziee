import { readFileSync } from 'fs'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedLiteratureResult, sampleResult } from './fixtures/mock-literature-result'

// Deterministic screening flow — seed a `literature_search` tool_result with
// typed structured_content (no live LLM), open the screening right-panel from
// the inline card, screen rows, watch PRISMA counts, and export. (Reload
// persistence relies on a real persisted conversation + localStorage panel
// snapshot — exercised by the updateRightPanelTab unit path, not this mock.)

test.describe('Literature screening flow', () => {
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

  test('open screening → screen → PRISMA counts → export', async ({ page, testInfra }) => {
    await seedLiteratureResult(page, testInfra.baseURL, sampleResult())

    // The inline card renders for the literature_search result.
    const openBtn = page.getByRole('button', { name: /Open in screening/ })
    await expect(openBtn).toBeVisible({ timeout: 10000 })
    await openBtn.click()

    // The right-panel screening workbench opens. The panel numbers each record
    // ("1. <title>") — assert the panel-unique prefix, not the bare title (which
    // also appears in the inline card → strict-mode multi-match).
    await expect(page.getByRole('heading', { name: 'Screening' })).toBeVisible({ timeout: 10000 })
    await expect(page.getByText('1. Base editing reduces off-target effects')).toBeVisible()
    await expect(page.getByText('After dedup: 2')).toBeVisible()
    // Completeness banner (labeled saturation, never a recall %).
    await expect(page.getByText(/Saturation estimate: MODERATE/i)).toBeVisible()

    // Bulk include both rows → PRISMA Included updates.
    await page.getByRole('checkbox', { name: /Select all|selected/ }).click()
    await page.getByRole('button', { name: 'Include', exact: true }).click()
    await expect(page.getByText('Included: 2')).toBeVisible()

    // Bulk exclude → exclusion-reason inputs appear; capture a reason.
    await page.getByRole('checkbox', { name: /Select all|selected/ }).click()
    await page.getByRole('button', { name: 'Exclude', exact: true }).click()
    await expect(page.getByText('Excluded: 2')).toBeVisible()
    const reason = page.getByPlaceholder('Exclusion reason (optional)').first()
    await expect(reason).toBeVisible()
    await reason.fill('out of scope')

    // Export → CSV download. Scope to the PANEL's export button ("Export all" /
    // "Export included") — a bare /Export/ also matches the chat-conversation
    // export button (chat/extensions/export), present in the conversation.
    await page.getByRole('button', { name: /Export (all|included)/ }).click()
    const download = page.waitForEvent('download')
    await page.getByRole('menuitem', { name: 'Export CSV' }).click()
    const file = await download
    expect(file.suggestedFilename()).toBe('screening.csv')

    // The CSV carries the typed exclusion reason — even though it was never
    // blurred (the export merges in-progress reason drafts; regression guard for
    // the draft/flush split silently dropping unsaved reasons).
    const path = await file.path()
    const csv = readFileSync(path, 'utf8')
    expect(csv).toContain('out of scope')
    expect(csv).toContain('Base editing reduces off-target effects')
  })

  test('Unscreen bulk action returns included rows to unscreened', async ({
    page,
    testInfra,
  }) => {
    await seedLiteratureResult(page, testInfra.baseURL, sampleResult())

    await page.getByRole('button', { name: /Open in screening/ }).click()
    await expect(page.getByRole('heading', { name: 'Screening' })).toBeVisible({
      timeout: 10000,
    })

    // Include both rows.
    await page.getByRole('checkbox', { name: /Select all|selected/ }).click()
    await page.getByRole('button', { name: 'Include', exact: true }).click()
    await expect(page.getByText('Included: 2')).toBeVisible()

    // Now Unscreen them → PRISMA Included drops back to 0.
    await page.getByRole('checkbox', { name: /Select all|selected/ }).click()
    await page.getByRole('button', { name: 'Unscreen', exact: true }).click()
    await expect(page.getByText('Included: 0')).toBeVisible()
  })

  test('inline result card shows the identified/dedup counts + saturation', async ({
    page,
    testInfra,
  }) => {
    await seedLiteratureResult(page, testInfra.baseURL, sampleResult())

    // The inline LiteratureToolResultCard digest line (BEFORE opening
    // screening) reports the identified/after-dedup counts and the
    // completeness/saturation estimate.
    await expect(
      page.getByText(/identified, 2 after dedup/),
    ).toBeVisible({ timeout: 10000 })
    await expect(page.getByText(/saturation: MODERATE/i)).toBeVisible()
    // The "Open in screening (N)" affordance carries the record count.
    await expect(
      page.getByRole('button', { name: /Open in screening \(2\)/ }),
    ).toBeVisible()
  })
})
