import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { byTestId } from '../testid'

/**
 * Split-chat E2E — per-pane KB citation source panel + highlight scope (TEST-74,
 * ITEM-49). A KB citation opened in ONE split pane mounts the kb_source viewer —
 * and its per-pane FileHighlightScope provider — in THAT pane's right panel only;
 * the other pane's right panel does NOT surface it. This exercises the real
 * citation → KbSourcePanel → scoped-highlight path in the correct pane (the only UI
 * path that mounts the scope provider); the scoped-key isolation itself is proven
 * at unit tier (TEST-73). Real-LLM (local bridge) — soft-skips when no bridge is
 * configured, exactly like tests/e2e/14-knowledge-base/kb-citation-flow.spec.ts.
 */
const isPlaceholder = (v: string | undefined): boolean =>
  v == null ||
  v.trim().length === 0 ||
  /^(sk-)?(xxx+|placeholder|changeme|test|dummy|your[-_]|<.*>)/i.test(v.trim())
const BRIDGE_URL = process.env.ZIEE_TEST_LLM_BASE_URL ?? process.env.OPENAI_BASE_URL
const BRIDGE_MODEL = process.env.ZIEE_TEST_LLM_MODEL ?? ''
const BRIDGE_KEY = process.env.OPENAI_API_KEY ?? process.env.ZIEE_TEST_LLM_API_KEY
const HAS_REAL_LLM =
  BRIDGE_URL != null && BRIDGE_URL.trim().length > 0 &&
  BRIDGE_MODEL.trim().length > 0 &&
  !isPlaceholder(BRIDGE_KEY)
const realLlmTest = HAS_REAL_LLM ? test : test.skip

test.describe('Split chat — per-pane KB citation source panel', () => {
  realLlmTest('a KB citation opened in pane B mounts kb_source in pane B only', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(200_000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }

    const providerId = await createProviderViaAPI(apiURL, token, 'Bridge', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)

    // Tool-capable model (search_knowledge only reaches a tool-flagged model).
    const model = await page.evaluate(async ([api, t, pid, name]) => {
      const r = await fetch(`${api}/api/llm-models`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${t}` },
        body: JSON.stringify({
          provider_id: pid, name, display_name: 'Bridge', enabled: true,
          engine_type: 'none', file_format: 'gguf',
          capabilities: { chat: true, completion: true, tools: true, streaming: true },
        }),
      })
      return r.json()
    }, [apiURL, token, providerId, BRIDGE_MODEL] as const)
    const modelId: string = model.id

    // KB + a synthetic-fact doc, attached; wait until indexed.
    const kb = await (await page.request.post(`${apiURL}/api/knowledge-bases`, { headers: auth, data: { name: 'Split Cite KB' } })).json()
    const fileForm = { name: 'beacon.txt', mimeType: 'text/plain', buffer: Buffer.from('Lab note. The Quintal beacon reads exactly 55231 hertz. End.') }
    const up = await page.request.post(`${apiURL}/api/files/upload`, { headers: { Authorization: `Bearer ${token}` }, multipart: { file: fileForm } })
    const fileId: string = (await up.json()).id
    await page.request.post(`${apiURL}/api/knowledge-bases/${kb.id}/documents`, { headers: auth, data: { file_ids: [fileId] } })
    await expect.poll(async () => {
      const docs = await (await page.request.get(`${apiURL}/api/knowledge-bases/${kb.id}/documents`, { headers: auth })).json()
      return docs?.[0]?.index_status
    }, { timeout: 20_000 }).toBe('indexed')

    // Two conversations, both with the model + KB attached.
    const mkConv = async () => {
      const c = await (await page.request.post(`${apiURL}/api/conversations`, { headers: auth, data: { model_id: modelId } })).json()
      await page.request.put(`${apiURL}/api/conversations/${c.id}/knowledge-bases/${kb.id}`, { headers: auth })
      return c.id as string
    }
    const convA = await mkConv()
    const convB = await mkConv()

    // [A | B] split via the picker.
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    const paneBInput = pane1.locator('textarea[placeholder*="Type your message"]')
    await expect(paneBInput).toBeVisible({ timeout: 15000 })

    // Ask in pane B → the tool-capable model fires search_knowledge → citation chip.
    await pane1.click()
    await paneBInput.fill('What does the Quintal beacon read? Use the knowledge base.')
    await pane1.getByTestId('chat-input-send-btn').click()

    // The model emits an inline `[n]` citation → a chip renders in pane B's answer.
    // Pane B is a non-native-scroll split pane, so the freshly-streamed answer can
    // sit below the fold (lazy-loaded, rendered-but-not-in-view); wait for the chip
    // to EXIST, then scroll it into pane B's viewport before interacting.
    const chip = pane1.locator('[data-testid^="kb-citation-chip-"]').first()
    await expect(chip).toHaveCount(1, { timeout: 150_000 })
    await chip.scrollIntoViewIfNeeded()
    await expect(chip).toBeVisible({ timeout: 10_000 })
    await chip.click()
    await expect(pane1.getByTestId('kb-tool-result-hits')).toBeVisible({ timeout: 10_000 })

    // "Open source" → the kb_source viewer opens in PANE B's right panel...
    await pane1.locator('[data-testid^="kb-hit-open-"]').first().click()
    await expect(pane1.getByTestId('chat-right-panel')).toBeVisible({ timeout: 15000 })
    await expect(pane1.getByTestId('chat-right-panel-tab-list')).toContainText('beacon.txt')

    // ...and NOT in pane A's — the source panel (and its per-pane highlight scope)
    // mounts in the pane whose citation was clicked, never the other pane's.
    await expect(
      pane0.getByTestId('chat-right-panel-tab-list').filter({ hasText: 'beacon.txt' }),
    ).toHaveCount(0)
  })
})
