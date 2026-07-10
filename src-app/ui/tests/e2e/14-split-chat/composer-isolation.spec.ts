import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Split-chat E2E — per-pane composer isolation (TEST-18 / TEST-31, and the
 * focus-routing basis for TEST-10 / TEST-24 / TEST-34). The MODEL selection is
 * per-pane (re-keyed by conversation id, DRIFT-1.2): selecting a different model
 * in each pane keeps each pane's own choice. Draft text is per-pane (per-pane
 * TextStore). File/assistant/MCP follow the FOCUSED pane (DRIFT-1.3/1.4), so a
 * send routes to the focused pane's context — verified here via focus-then-read.
 */
test.describe('Split chat — per-pane composer (model + draft) isolation', () => {
  test('each pane keeps its own model selection and its own draft', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    // Two distinctly-named models so we can tell the two panes' selections apart.
    const alphaId = await createModelViaAPI(apiURL, token, providerId, 'model-alpha', 'Model Alpha', 'openai')
    const betaId = await createModelViaAPI(apiURL, token, providerId, 'model-beta', 'Model Beta', 'openai')

    const res = await page.request.post(`${apiURL}/api/conversations`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { title: 'Composer Iso A' },
    })
    const convA = (await res.json()).id as string

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')

    // Split: pane 0 = conv A, pane 1 = a fresh new-chat pane (a DIFFERENT
    // selection key, so the two panes' model choices are independent).
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane0).toBeVisible({ timeout: 15000 })
    await expect(pane1).toBeVisible({ timeout: 15000 })

    // Both panes start at the same default model (Model Alpha, the first).
    await expect(pane0.getByTestId('model-selector')).toContainText('Alpha')

    // Change ONLY pane 1 → Model Beta. (Open just one select so we drive exactly
    // one base-ui listbox portal; options carry a per-model `-opt-<id>` testid.)
    await pane1.getByTestId('ullm-model-select').click()
    await page.locator(`[data-testid="ullm-model-select-opt-${betaId}"]`).click()

    // pane 1 now shows Beta; pane 0 is UNCHANGED (still Alpha) — per-conversation
    // re-keyed selection. If it were shared, retargeting pane 1 would have moved
    // pane 0 to Beta too.
    await expect(pane1.getByTestId('model-selector')).toContainText('Beta')
    await expect(pane0.getByTestId('model-selector')).toContainText('Alpha')
    await expect(pane0.getByTestId('model-selector')).not.toContainText('Beta')
    void alphaId

    // Drafts are per-pane too (per-pane TextStore).
    const inputA = pane0.locator('textarea[placeholder*="Type your message"]')
    const inputB = pane1.locator('textarea[placeholder*="Type your message"]')
    await inputA.fill('alpha draft')
    await inputB.fill('beta draft')
    await expect(inputA).toHaveValue('alpha draft')
    await expect(inputB).toHaveValue('beta draft')
    // Re-assert the model selection survived the draft edits (still isolated).
    await expect(pane0.getByTestId('model-selector')).toContainText('Alpha')
    await expect(pane1.getByTestId('model-selector')).toContainText('Beta')
  })
})
