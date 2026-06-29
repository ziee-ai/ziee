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

/**
 * E2E — RIS + BibTeX export from the literature screening panel.
 *
 * Audit gap: `screening-flow.spec.ts` exercises the CSV export menu item
 * only; the RIS and BibTeX items (LiteratureScreeningPanel.tsx:180-183 →
 * citationFormats.ts toRis/toBibtex) had no E2E. This seeds a deterministic
 * literature_search result, opens screening, and downloads each format,
 * asserting the filename + the format's structural markers.
 */

test.describe('Literature — RIS/BibTeX export', () => {
  test.describe.configure({ retries: 2 })

  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await page.evaluate(() =>
      JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(
      apiURL,
      token,
      'OpenAI',
      'openai',
    )
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
  })

  test('exports RIS then BibTeX with the right filename + content', async ({
    page,
    testInfra,
  }) => {
    await seedLiteratureResult(page, testInfra.baseURL, sampleResult())

    await byTestId(page, 'lit-tool-result-open-button').click()
    await expect(byTestId(page, 'lit-screening-panel')).toBeVisible({ timeout: 10000 })

    // ── RIS ──
    await byTestId(page, 'lit-screening-export-button').click()
    const risDownload = page.waitForEvent('download')
    await byTestId(page, 'lit-screening-export-dropdown-item-ris').click()
    const risFile = await risDownload
    expect(risFile.suggestedFilename()).toBe('screening.ris')
    const risStream = await risFile.createReadStream()
    const risText = (await streamToString(risStream)) ?? ''
    expect(risText).toMatch(/^TY {2}- /m)
    expect(risText).toMatch(/^ER {2}- /m)

    // ── BibTeX ──
    await byTestId(page, 'lit-screening-export-button').click()
    const bibDownload = page.waitForEvent('download')
    await byTestId(page, 'lit-screening-export-dropdown-item-bibtex').click()
    const bibFile = await bibDownload
    expect(bibFile.suggestedFilename()).toBe('screening.bib')
    const bibStream = await bibFile.createReadStream()
    const bibText = (await streamToString(bibStream)) ?? ''
    expect(bibText).toMatch(/@(article|misc)\{/)
    expect(bibText).toMatch(/title = \{/)
  })
})

async function streamToString(
  stream: NodeJS.ReadableStream | null,
): Promise<string | null> {
  if (!stream) return null
  const chunks: Buffer[] = []
  for await (const chunk of stream) {
    chunks.push(Buffer.from(chunk))
  }
  return Buffer.concat(chunks).toString('utf8')
}
