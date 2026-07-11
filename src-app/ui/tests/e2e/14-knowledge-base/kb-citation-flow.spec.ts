import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { byTestId } from '../testid'

// TEST-40 (ITEM-35,36,37): the citation flow — a tool-capable model fires
// search_knowledge, the transparency card renders the retrieved passage, and
// "Open source" opens the right-panel kb_source viewer. Real-LLM (local bridge).
//
// A real-LLM e2e cannot run without a live bridge, and this box's `.env.test`
// ships PLACEHOLDER keys (`sk-xxx…`), so the spec must soft-skip unless a real
// bridge is configured — the codebase-standard real-LLM gate. This is NOT a
// hard-ignore green-wash: when the bridge is absent/placeholder the test
// is declared skipped, so it reports as SKIPPED (visible in the run),
// never a silent pass. When a real bridge IS configured it runs for real.
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
// `test` when a real bridge exists, `test.skip` (reports SKIPPED) otherwise.
const realLlmTest = HAS_REAL_LLM ? test : test.skip

test.describe('Knowledge Base — citation flow (real LLM)', () => {
  realLlmTest('search_knowledge card renders + Open source opens the kb_source panel', async ({ page, testInfra }) => {
    test.setTimeout(180_000)
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }

    // provider (local bridge via the OPENAI_BASE_URL/ZIEE_TEST_LLM_BASE_URL seam) + access
    const providerId = await createProviderViaAPI(apiURL, token, 'Bridge', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)

    // a TOOL-CAPABLE model (the helper doesn't set tools; search_knowledge only
    // reaches a tool-flagged model — else it hallucinates the call).
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

    // KB + a synthetic-fact doc, attached; wait until FTS-indexed.
    const kb = await (await page.request.post(`${apiURL}/api/knowledge-bases`, { headers: auth, data: { name: 'Cite KB' } })).json()
    const fileForm = { name: 'fact.txt', mimeType: 'text/plain', buffer: Buffer.from('Lab note. The Quintal beacon reads exactly 55231 hertz. End.') }
    const up = await page.request.post(`${apiURL}/api/files/upload`, { headers: { Authorization: `Bearer ${token}` }, multipart: { file: fileForm } })
    const fileId: string = (await up.json()).id
    await page.request.post(`${apiURL}/api/knowledge-bases/${kb.id}/documents`, { headers: auth, data: { file_ids: [fileId] } })
    await expect.poll(async () => {
      const docs = await (await page.request.get(`${apiURL}/api/knowledge-bases/${kb.id}/documents`, { headers: auth })).json()
      return docs?.[0]?.index_status
    }, { timeout: 20_000 }).toBe('indexed')

    // conversation with the model + the KB attached.
    const conv = await (await page.request.post(`${apiURL}/api/conversations`, { headers: auth, data: { model_id: modelId } })).json()
    await page.request.put(`${apiURL}/api/conversations/${conv.id}/knowledge-bases/${kb.id}`, { headers: auth })

    // Ask about the synthetic fact in the real UI.
    await page.goto(`${baseURL}/chat/${conv.id}`)
    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await expect(textarea).toBeVisible({ timeout: 15000 })
    await textarea.fill('What does the Quintal beacon read? Use the knowledge base.')
    await byTestId(page, 'chat-input-send-btn').click()

    // The tool-capable model fires search_knowledge → a transparency card
    // renders. (The model may search MORE THAN ONCE in a turn → multiple cards;
    // `.first()` avoids a strict-locator error, and the chip resolves to the
    // most-recent card by design.)
    await expect(byTestId(page, 'kb-tool-result-card').first()).toBeVisible({ timeout: 120_000 })

    // FB-14: cards are DEFAULT-COLLAPSED — no passages shown until expanded.
    await expect(byTestId(page, 'kb-tool-result-hits')).toHaveCount(0)

    // FB-11 (the true test): the REAL model, under the grounding prompt, emits
    // inline `[n]` citation markers in its answer → the tokenizer + `a`-override
    // render them as clickable citation chips. If the model never emits `[n]`,
    // no chip appears and THIS FAILS (a real finding, not skipped).
    const chip = page.locator('[data-testid^="kb-citation-chip-"]').first()
    await expect(chip).toBeVisible({ timeout: 90_000 })
    // Clicking the chip expands the transparency card + reveals the passages
    // (FB-11 chip action + FB-14 expand-on-demand, in one).
    await chip.click()
    await expect(byTestId(page, 'kb-tool-result-hits')).toBeVisible()

    // "Open source" opens the right-panel kb_source viewer (a tab for the cited doc).
    await page.locator('[data-testid^="kb-hit-open-"]').first().click()
    await expect(byTestId(page, 'chat-right-panel').first()).toBeVisible({ timeout: 15000 })
    await expect(byTestId(page, 'chat-right-panel-tab-list')).toContainText('fact.txt')
  })
})
