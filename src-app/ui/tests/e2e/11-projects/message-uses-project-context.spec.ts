import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
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
 *   project via API → navigate to /chat?project_id=<uuid> →
 *   send message in the UI → wait for streamed response → assert
 *   response contains the magic markers that prove project context
 *   was injected end-to-end.
 *
 * Equivalent assertions live in the Rust Tier-3 tests at
 * `src-app/server/tests/project/injection_test.rs`. This spec adds
 * the UI-layer proof that a user clicking through the real
 * affordances (URL latch → ChatInput) ends up with project context
 * on the wire.
 *
 * **Gating**: tests soft-skip when `ANTHROPIC_API_KEY` is unset. The
 * shared `createProviderViaAPI` helper reads the env via
 * `process.env.ANTHROPIC_API_KEY`; without the key, real-LLM calls
 * would fail. We pre-check + `test.skip()` to keep the suite green
 * without API access.
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

    // Navigate to /chat?project_id=<uuid> — same route the
    // ProjectDetailPage's "New chat" button uses.
    await page.goto(`${baseURL}/chat?project_id=${project.id}`)
    await page.waitForLoadState('networkidle')

    // Send a message via the real UI.
    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await expect(textarea).toBeVisible({ timeout: 10000 })
    await textarea.fill('Say hello.')

    const sendButton = page.getByRole('button', { name: 'Send message' })
    await expect(sendButton).toBeEnabled({ timeout: 10000 })
    await sendButton.click()

    // Wait for the streamed assistant response. The chat layout
    // renders assistant text in the conversation pane — we wait for
    // the magic marker to appear anywhere on the page.
    await expect(page.locator('body')).toContainText('ZZZ_E2E_BEACON_77', {
      timeout: 45000,
    })

    // Visual proof the header chip correctly identifies the
    // conversation's project (renders only when conversation.project_id
    // is set, which it should be after the URL-latch flow ran).
    await expect(page.getByText(/In project: E2E Magic Project/i)).toBeVisible(
      { timeout: 5000 },
    )
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
    await page.waitForLoadState('networkidle')

    const textarea = page.locator('textarea[placeholder*="Type your message"]')
    await expect(textarea).toBeVisible({ timeout: 10000 })
    await textarea.fill('Say hello.')

    const sendButton = page.getByRole('button', { name: 'Send message' })
    await expect(sendButton).toBeEnabled({ timeout: 10000 })
    await sendButton.click()

    // Wait for streaming to complete (Haiku is fast — 15s budget),
    // then assert the marker from the unattached project is absent.
    await page.waitForTimeout(15000)
    const body = await page.locator('body').textContent()
    expect(body ?? '').not.toContain('NEVER_E2E_LEAK_TOKEN_55')
  })
})
