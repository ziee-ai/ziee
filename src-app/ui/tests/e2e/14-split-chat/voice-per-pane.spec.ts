import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { installVoiceBrowserMocks, routeVoice } from '../14-voice/voice-helpers'

/**
 * Split-chat E2E — per-pane voice dictation (TEST-67, ITEM-45). Recording in one
 * split pane dictates into THAT pane's composer, not the focused pane; while a
 * pane records, the other pane's mic is disabled (A1 exclusive recording). Uses
 * the 14-voice browser + `/api/voice/**` mocks (no whisper runtime), so the
 * capture APIs + transcription are deterministic.
 */
test.describe('Split chat — per-pane voice dictation', () => {
  test.describe.configure({ retries: 1 })

  test('recording in pane B dictates into pane B; pane A stays empty + its mic disables', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await installVoiceBrowserMocks(page)
    const voice = await routeVoice(page)
    voice.setTranscribe({ text: 'dictation into pane bravo', language: 'en', duration_ms: 500 })

    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
    const auth = { Authorization: `Bearer ${token}` }
    const mkConv = async (t: string) =>
      (
        await (
          await page.request.post(`${apiURL}/api/conversations`, { headers: auth, data: { title: t } })
        ).json()
      ).id as string
    const convA = await mkConv('Voice A')
    const convB = await mkConv('Voice B')

    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').click()
    const pane0 = byTestId(page, 'chat-pane-0')
    const pane1 = byTestId(page, 'chat-pane-1')
    await expect(pane1).toBeVisible({ timeout: 15000 })
    await pane1.getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(
      pane1.locator('textarea[placeholder*="Type your message"]'),
    ).toBeVisible({ timeout: 15000 })

    // Both panes show their own mic (per-pane VoiceStore instances).
    await expect(pane0.getByTestId('voice-mic-button')).toBeVisible({ timeout: 15000 })
    await expect(pane1.getByTestId('voice-mic-button')).toBeVisible()

    // Start recording in pane B (no prior focus-click on its textarea).
    await pane1.getByTestId('voice-mic-button').click()
    // Pane B is recording (elapsed timer); pane A's mic disables (A1 exclusive).
    await expect(pane1.getByTestId('voice-elapsed')).toBeVisible({ timeout: 15000 })
    await expect(pane0.getByTestId('voice-mic-button')).toBeDisabled()

    // Stop → transcribe → the transcript lands in pane B's textarea, NOT pane A's.
    await pane1.getByTestId('voice-mic-button').click() // now the Stop control
    await expect(
      pane1.locator('textarea[placeholder*="Type your message"]'),
    ).toHaveValue(/dictation into pane bravo/, { timeout: 30000 })
    await expect(
      pane0.locator('textarea[placeholder*="Type your message"]'),
    ).toHaveValue('')
    // Pane A's mic re-enables once B's recording flow finished.
    await expect(pane0.getByTestId('voice-mic-button')).toBeEnabled({ timeout: 15000 })
  })

  // TEST-67b (audit #8): closing a NON-recording pane must not cancel the
  // recording pane's session (the recorder is module-level; the ITEM-45 fix made
  // error-state/unmount cleanup own-pane-only). 3 panes so closing one keeps the
  // recording pane's component instance (no collapse-to-single remount).
  test('closing a non-recording pane does NOT cancel the recording pane', async ({
    page,
    testInfra,
  }) => {
    test.setTimeout(120000)
    const { baseURL, apiURL } = testInfra
    await installVoiceBrowserMocks(page)
    const voice = await routeVoice(page)
    voice.setTranscribe({ text: 'still recording after close', language: 'en', duration_ms: 500 })

    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
    const auth = { Authorization: `Bearer ${token}` }
    const mkConv = async (t: string) =>
      (await (await page.request.post(`${apiURL}/api/conversations`, { headers: auth, data: { title: t } })).json()).id as string
    const convA = await mkConv('VClose A')
    const convB = await mkConv('VClose B')
    const convC = await mkConv('VClose C')

    // Build [A | B | C].
    await page.goto(`${baseURL}/chat/${convA}`)
    await page.waitForLoadState('load')
    await byTestId(page, 'chat-split-btn').first().click()
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 })
    await byTestId(page, 'chat-pane-1').getByTestId(`conversation-picker-item-${convB}`).click()
    await expect(byTestId(page, 'chat-pane-1').locator('textarea[placeholder*="Type your message"]')).toBeVisible({ timeout: 15000 })
    await byTestId(page, 'chat-pane-1').getByTestId('chat-split-btn').click()
    await expect(byTestId(page, 'chat-pane-2')).toBeVisible({ timeout: 15000 })
    await byTestId(page, 'chat-pane-2').getByTestId(`conversation-picker-item-${convC}`).click()
    await expect(byTestId(page, 'chat-pane-2').locator('textarea[placeholder*="Type your message"]')).toBeVisible({ timeout: 15000 })

    // Record in pane 1 (B, the middle pane).
    await byTestId(page, 'chat-pane-1').getByTestId('voice-mic-button').click()
    await expect(byTestId(page, 'chat-pane-1').getByTestId('voice-elapsed')).toBeVisible({ timeout: 15000 })

    // Close pane 0 (A) — a NON-recording pane. Panes reindex to [B | C]; B → pane 0.
    await byTestId(page, 'chat-pane-0').getByTestId('chat-pane-close').click()
    await expect(byTestId(page, 'chat-pane-1')).toBeVisible({ timeout: 15000 }) // still split (C)

    // B's recording SURVIVED the close (B is now pane 0). Its elapsed timer still
    // runs — closing the non-recording pane didn't cancel or strand it.
    const paneB = byTestId(page, 'chat-pane-0')
    await expect(paneB.getByTestId('conversation-title')).toContainText('VClose B')
    await expect(paneB.getByTestId('voice-elapsed')).toBeVisible()

    // Stopping B still transcribes into B's composer — the session was intact.
    await paneB.getByTestId('voice-mic-button').click()
    await expect(paneB.locator('textarea[placeholder*="Type your message"]')).toHaveValue(
      /still recording after close/,
      { timeout: 30000 },
    )
  })
})
