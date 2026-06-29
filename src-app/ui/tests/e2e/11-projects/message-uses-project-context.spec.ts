import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * Real-LLM E2E for project context injection.
 *
 * Drives the full UI flow:
 *   login → configure Anthropic provider (real API key) → seed
 *   project via API → navigate to /projects/{id} → send message
 *   in the inline ChatInput → wait for streamed response → assert
 *   response contains the magic markers that prove project context
 *   was injected end-to-end.
 *
 * The `/chat?project_id=<uuid>` URL pattern is gone (chat doesn't
 * know about projects anymore); the project detail page now owns
 * the "start a new chat in this project" affordance via the
 * embedded ChatInput, and the project chat extension's
 * `afterCreateConversation` hook files the conversation into the
 * project on first send.
 *
 * Equivalent assertions live in the Rust Tier-3 tests at
 * `src-app/server/tests/project/injection_test.rs`. This spec adds
 * the UI-layer proof that a user clicking through the real
 * affordances ends up with project context on the wire.
 *
 * **Gating**: tests soft-skip when `ANTHROPIC_API_KEY` is unset.
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''
const HAS_ANTHROPIC = ANTHROPIC_KEY.length > 0

test.describe('Projects - message uses project context (real LLM)', () => {
  test.skip(
    !HAS_ANTHROPIC,
    'ANTHROPIC_API_KEY not set — real-LLM E2E tests skipped',
  )

  test('project instructions reach the LLM (response contains magic marker)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // Configure a real Anthropic provider + Haiku model. The helper
    // reads ANTHROPIC_API_KEY from env.
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

    // Create a project via API with the magic instruction.
    const project = await page.evaluate(
      async ([api, t]) => {
        const r = await fetch(`${api}/api/projects`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${t}`,
          },
          body: JSON.stringify({
            name: 'E2E Magic Project',
            instructions:
              "You are required to begin every response with the exact " +
              "literal string 'ZZZ_E2E_BEACON_77' (no preface). After " +
              "that token you can respond normally.",
          }),
        })
        return await r.json()
      },
      [apiURL, adminToken],
    )
    expect(project).toHaveProperty('id')

    // Navigate to the project detail page; the inline ChatInput
    // there is the entry point for new project chats. (The legacy
    // `/chat?project_id=<uuid>` URL still redirects here for
    // backward-compat, but the canonical flow is the project page.)
    await page.goto(`${baseURL}/projects/${project.id}`)
    await page.waitForLoadState('load')

    // Send a message via the real UI — the project chat extension's
    // afterCreateConversation hook attaches the new conversation to
    // the project before the message stream starts, so the system
    // prompt carries the project's instructions.
    const textarea = page.locator(
      '[data-test-section="chat-input"] textarea[placeholder*="Type your message"]',
    )
    await expect(textarea).toBeVisible({ timeout: 10000 })
    await textarea.fill('Say hello.')

    const sendButton = byTestId(page, 'chat-input-send-btn')
    await expect(sendButton).toBeEnabled({ timeout: 10000 })
    await sendButton.click()

    // After send, the page navigates to the namespaced URL once the
    // attach call resolves. Confirm the routing contract first so a
    // failure here points at the wiring instead of at the LLM.
    await page.waitForURL(
      new RegExp(`/projects/${project.id}/chat/[0-9a-f-]+`),
      { timeout: 30000 },
    )

    // Wait for the streamed assistant response — magic marker proves
    // project instructions reached the LLM.
    await expect(page.locator('body')).toContainText('ZZZ_E2E_BEACON_77', {
      timeout: 45000,
    })

    // Visual proof the header chip correctly identifies the
    // conversation's project.
    await expect(byTestId(page, 'project-header-chip-tag')).toBeVisible({
      timeout: 5000,
    })
  })

  test('unfiled conversation does NOT receive project context', async ({
    page,
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

    // Project exists with a magic marker — but we NEVER attach a
    // conversation to it.
    await page.evaluate(
      async ([api, t]) => {
        await fetch(`${api}/api/projects`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${t}`,
          },
          body: JSON.stringify({
            name: 'Unattached Project',
            instructions:
              "Always start your response with NEVER_E2E_LEAK_TOKEN_55.",
          }),
        })
      },
      [apiURL, adminToken],
    )

    // Plain /chat — no project_id query param.
    await page.goto(`${baseURL}/chat`)
    await page.waitForLoadState('load')

    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await expect(textarea).toBeVisible({ timeout: 10000 })
    await textarea.fill('Say hello.')

    const sendButton = byTestId(page, 'chat-input-send-btn')
    await expect(sendButton).toBeEnabled({ timeout: 10000 })
    await sendButton.click()

    // Wait for streaming to complete (Haiku is fast — 15s budget),
    // then assert the marker from the unattached project is absent.
    await page.waitForTimeout(15000)
    const body = await page.locator('body').textContent()
    expect(body ?? '').not.toContain('NEVER_E2E_LEAK_TOKEN_55')
  })
})
