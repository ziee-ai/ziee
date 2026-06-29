import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { seedLiteratureResult, sampleResult } from './fixtures/mock-literature-result'
import { byTestId } from '../testid'

// audit id all-2e73f996b353 — screening decisions persist across a page reload
// (LiteratureScreeningPanel persists decisions via updateRightPanelTab →
// localStorage 'ziee-right-panel-tabs-v2'); the existing screening-flow spec
// explicitly does NOT cover reload persistence. The route-mocked /messages
// reload survives page.reload(), so the inline card + panel re-render and the
// panel must restore the previously-made decisions from localStorage.
test.describe('Literature screening — persistence across reload', () => {
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

  test('include decisions survive a reload', async ({ page, testInfra }) => {
    await seedLiteratureResult(page, testInfra.baseURL, sampleResult())

    // Open the screening panel and include both records.
    await byTestId(page, 'lit-tool-result-open-button').click()
    await expect(byTestId(page, 'lit-screening-panel')).toBeVisible({ timeout: 10000 })
    await byTestId(page, 'lit-screening-select-all-checkbox').click()
    await byTestId(page, 'lit-screening-bulk-include-button').click()
    await expect(byTestId(page, 'lit-screening-tag-included')).toContainText('2')

    // The decisions are persisted to localStorage.
    const persisted = await page.evaluate(() =>
      localStorage.getItem('ziee-right-panel-tabs-v2'),
    )
    expect(persisted, 'screening state must be persisted to localStorage').toBeTruthy()

    // Reload: the route-mocked conversation re-renders; reopen the panel and the
    // include decisions must be restored (Included count preserved).
    await page.reload()
    await page.waitForLoadState('domcontentloaded')
    await byTestId(page, 'lit-tool-result-open-button').click()
    await expect(byTestId(page, 'lit-screening-panel')).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'lit-screening-tag-included')).toContainText('2', {
      timeout: 15000,
    })
  })
})
