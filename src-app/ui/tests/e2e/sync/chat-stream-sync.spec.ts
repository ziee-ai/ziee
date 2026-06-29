import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

// Realtime sync for LIVE CHAT TOKEN STREAMING across devices.
//
// The conversation list dimension (sidebar create/rename/delete cross-window)
// is covered by `conversation-sync.spec.ts`. The token stream itself is
// covered deterministically at the backend by `chat_stream_test.rs`
// (stub-engine-backed). This spec closes the UI layer for the streaming
// dimension: two browser contexts viewing the SAME conversation; device A
// sends, device B's transcript renders the streamed assistant reply WITHOUT
// the user on B doing anything.
//
// Per the chat-stream architecture, each authenticated SSE connection
// subscribes to specific conversation ids (`POST /chat-stream/subscribe`),
// and the per-user fan-out delivers token frames to every subscribed
// connection. So when A sends and B's connection is subscribed to the same
// conversation, B's transcript receives the same frame stream as A's.
//
// Why a REAL provider (and not a stub):
//
//   E2E backends spawned by `tests/fixtures/test-context.ts` don't ship
//   the stub-engine binary; wiring it requires test-infra changes the
//   sync suite intentionally avoids (see the NOTE in
//   `conversation-sync.spec.ts`). Anthropic's claude-haiku is cheap
//   (~$0.0005 per turn), fast, and already configured for other Tier-3
//   real-LLM specs in this repo (projects/message-uses-project-context).
//
// Soft-skipped when `ANTHROPIC_API_KEY` is unset so the suite stays green on
// developers without keys; the gating mirrors the project-context spec.
//
// Run with --workers=1.

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''
const HAS_ANTHROPIC = ANTHROPIC_KEY.length > 0

test.describe('Realtime sync — chat token stream (cross-device)', () => {
  test.skip(
    !HAS_ANTHROPIC,
    'ANTHROPIC_API_KEY not set — real-LLM cross-device chat stream skipped',
  )

  test('a message sent on device A streams its reply into device B viewing the same conversation', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // ── Setup: provider + model ─────────────────────────────────────
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'Anthropic',
      'anthropic',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    // ── Create a conversation via API so we have an id BEFORE either
    //    device subscribes to its stream ───────────────────────────
    const convRes = await page.request.post(
      `${baseURL}/api/conversations`,
      {
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${adminToken}`,
        },
        data: { title: `XSync Stream ${Date.now()}` },
      },
    )
    expect(convRes.ok()).toBeTruthy()
    const conv = await convRes.json()
    const convId = conv.id as string

    // ── Device A opens the conversation ──────────────────────────
    await page.goto(`${baseURL}/chat/${convId}`)
    await page.waitForSelector('textarea[placeholder*="Type your message"]', {
      timeout: 30_000,
    })

    // ── Device B opens the SAME conversation ─────────────────────
    // It must be open BEFORE A sends so its chat-stream connection is
    // subscribed to convId and ready to receive frames. (The Chat
    // store's onConversationLoad calls `chatStreamClient.subscribe(convId)`
    // immediately on mount.)
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await pageB.goto(`${baseURL}/chat/${convId}`)
      await pageB.waitForSelector(
        'textarea[placeholder*="Type your message"]',
        { timeout: 30_000 },
      )

      // ── Device A sends a message containing a marker the reply
      //    must echo back ──────────────────────────────────────
      // The marker is chosen so the LLM will naturally include it in
      // its response without specific prompting tricks: it's English-
      // y enough to flow but unique enough to disambiguate from any
      // model boilerplate.
      const marker = `XSYNC_BEACON_${Date.now()}`
      const textareaA = page.locator(
        'textarea[placeholder*="Type your message"]',
      )
      await textareaA.fill(
        `Reply with exactly this token and nothing else: ${marker}`,
      )
      const sendButtonA = byTestId(page, 'chat-input-send-btn')
      await expect(sendButtonA).toBeEnabled({ timeout: 10_000 })
      await sendButtonA.click()

      // ── The assertion: device B's transcript contains the marker
      //    without anyone on device B typing or clicking. Generous
      //    timeout to absorb model latency (haiku usually < 5s but
      //    cold-start + network can push it). ──────────────────
      await expect(pageB.locator('body')).toContainText(marker, {
        timeout: 60_000,
      })

      // Sanity: device A also got the same reply (the originator's
      // chat-stream connection is also subscribed). If this fires
      // BEFORE the B assertion above, both devices saw the stream.
      await expect(page.locator('body')).toContainText(marker, {
        timeout: 5_000,
      })
    } finally {
      await ctxB.close()
    }
  })

  test('a reply to a message sent on device B streams back into device A (bidirectional, second turn)', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'Anthropic',
      'anthropic',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    const convRes = await page.request.post(`${baseURL}/api/conversations`, {
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${adminToken}`,
      },
      data: { title: `XSync BiDir ${Date.now()}` },
    })
    expect(convRes.ok()).toBeTruthy()
    const convId = (await convRes.json()).id as string

    // Device A opens the conversation and stays passive for the second turn.
    await page.goto(`${baseURL}/chat/${convId}`)
    await page.waitForSelector('textarea[placeholder*="Type your message"]', {
      timeout: 30_000,
    })

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await pageB.goto(`${baseURL}/chat/${convId}`)
      await pageB.waitForSelector('textarea[placeholder*="Type your message"]', {
        timeout: 30_000,
      })

      // Turn 1 — device A sends; both devices converge (establishes the shared
      // transcript / subscription before the cross-direction turn).
      const markerA = `XSYNC_A_${Date.now()}`
      await page
        .locator('textarea[placeholder*="Type your message"]')
        .fill(`Reply with exactly this token and nothing else: ${markerA}`)
      const sendA = byTestId(page, 'chat-input-send-btn')
      await expect(sendA).toBeEnabled({ timeout: 10_000 })
      await sendA.click()
      await expect(pageB.locator('body')).toContainText(markerA, { timeout: 60_000 })

      // Turn 2 — now device B drives a SECOND turn in the same conversation;
      // device A (passive) must receive B's streamed reply, proving the stream
      // fan-out is bidirectional + persists across turns (not just A→B once).
      const markerB = `XSYNC_B_${Date.now()}`
      const textareaB = pageB.locator('textarea[placeholder*="Type your message"]')
      await expect(textareaB).toBeEnabled({ timeout: 30_000 })
      await textareaB.fill(`Reply with exactly this token and nothing else: ${markerB}`)
      const sendB = byTestId(pageB, 'chat-input-send-btn')
      await expect(sendB).toBeEnabled({ timeout: 10_000 })
      await sendB.click()

      await expect(page.locator('body')).toContainText(markerB, { timeout: 60_000 })
    } finally {
      await ctxB.close()
    }
  })
})
