import { readFileSync } from 'fs'
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
    const openBtn = byTestId(page, 'lit-tool-result-open-button')
    await expect(openBtn).toBeVisible({ timeout: 10000 })
    await openBtn.click()

    // The right-panel screening workbench opens. The records list carries the
    // (dynamic) seeded title.
    await expect(byTestId(page, 'lit-screening-panel')).toBeVisible({ timeout: 10000 })
    await expect(byTestId(page, 'lit-screening-records-list')).toContainText(
      'Base editing reduces off-target effects',
    )
    await expect(byTestId(page, 'lit-screening-tag-after-dedup')).toContainText('2')
    // Completeness banner (labeled saturation, never a recall %).
    await expect(byTestId(page, 'lit-screening-completeness')).toContainText('MODERATE')

    // Bulk include both rows → PRISMA Included updates.
    await byTestId(page, 'lit-screening-select-all-checkbox').click()
    await byTestId(page, 'lit-screening-bulk-include-button').click()
    await expect(byTestId(page, 'lit-screening-tag-included')).toContainText('2')

    // Bulk exclude → exclusion-reason inputs appear; capture a reason.
    await byTestId(page, 'lit-screening-select-all-checkbox').click()
    await byTestId(page, 'lit-screening-bulk-exclude-button').click()
    await expect(byTestId(page, 'lit-screening-tag-excluded')).toContainText('2')
    const reason = page.getByPlaceholder('Exclusion reason (optional)').first()
    await expect(reason).toBeVisible()
    await reason.fill('out of scope')

    // Export → CSV download via the PANEL's export dropdown.
    await byTestId(page, 'lit-screening-export-button').click()
    const download = page.waitForEvent('download')
    await byTestId(page, 'lit-screening-export-dropdown-item-csv').click()
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

  test('screening decisions persist across a page reload (draft/flush snapshot)', async ({
    page,
    testInfra,
  }) => {
    // screening-flow only asserted export-after-fill; the panel snapshot
    // persistence (decisions survive a reload via the serializable panel-tab
    // data) was untested. Screen both rows, reload, and assert the decisions
    // restore.
    await seedLiteratureResult(page, testInfra.baseURL, sampleResult())

    await byTestId(page, 'lit-tool-result-open-button').click()
    await expect(byTestId(page, 'lit-screening-panel')).toBeVisible({
      timeout: 10000,
    })

    // Include both rows → PRISMA "Included: 2".
    await byTestId(page, 'lit-screening-select-all-checkbox').click()
    await byTestId(page, 'lit-screening-bulk-include-button').click()
    await expect(byTestId(page, 'lit-screening-tag-included')).toContainText('2')

    // Reload — the conversation + panel snapshot restore from persistence.
    await page.reload()
    await page.waitForLoadState('domcontentloaded')

    // The panel may auto-restore; if not, re-open it from the inline card.
    const panel = byTestId(page, 'lit-screening-panel')
    if (!(await panel.isVisible().catch(() => false))) {
      await byTestId(page, 'lit-tool-result-open-button').click({ timeout: 15000 })
    }
    await expect(panel).toBeVisible({ timeout: 15000 })

    // The include decisions survived the reload.
    await expect(byTestId(page, 'lit-screening-tag-included')).toContainText('2', {
      timeout: 15000,
    })
  })

  test('per-row Segmented control sets an individual screening decision', async ({
    page,
    testInfra,
  }) => {
    // The other tests use the BULK Include/Exclude buttons; the per-row
    // Segmented decision control (LiteratureScreeningPanel.tsx:243-251) was
    // untested. Set ONE row to Include + ONE to Exclude via their Segmented.
    await seedLiteratureResult(page, testInfra.baseURL, sampleResult())
    await byTestId(page, 'lit-tool-result-open-button').click()
    await expect(byTestId(page, 'lit-screening-panel')).toBeVisible({ timeout: 10000 })

    await expect(byTestId(page, `lit-screening-record-decision-${KEY1}`)).toBeVisible({
      timeout: 10000,
    })

    // Row 1 → Include, Row 2 → Exclude, each via its own Segmented control.
    await byTestId(page, `lit-screening-record-decision-${KEY1}-opt-include`).click()
    await expect(byTestId(page, 'lit-screening-tag-included')).toContainText('1', {
      timeout: 10000,
    })

    await byTestId(page, `lit-screening-record-decision-${KEY2}-opt-exclude`).click()
    await expect(byTestId(page, 'lit-screening-tag-excluded')).toContainText('1', {
      timeout: 10000,
    })
    // The exclusion-reason input appears for the per-row Exclude decision.
    await expect(byTestId(page, `lit-screening-record-reason-${KEY2}`)).toBeVisible()
  })

  test('inline tool-result card shows the dedup + saturation estimate', async ({
    page,
    testInfra,
  }) => {
    await seedLiteratureResult(page, testInfra.baseURL, sampleResult())

    // The INLINE LiteratureToolResultCard (before opening screening) summarizes
    // the search as "<total> identified, <n> after dedup · saturation: <EST>".
    // The existing flow test only asserts the screening-PANEL banner; this
    // covers the inline card's own completeness/saturation line.
    const summary = byTestId(page, 'lit-tool-result-summary')
    await expect(summary).toBeVisible({ timeout: 10000 })
    // The inline summary carries the dedup count + the saturation estimate.
    await expect(summary).toContainText('2')
    await expect(summary).toContainText('MODERATE')
  })
})
