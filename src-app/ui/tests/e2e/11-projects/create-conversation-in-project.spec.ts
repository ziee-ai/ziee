import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  fillProjectForm,
  goToProjectsPage,
  openCreateProjectDrawer,
  submitProjectForm,
} from './helpers/project-helpers'

/**
 * Round-4 Option A redesign: the project detail page now embeds a
 * `<ChatInput>` directly, replacing the old "New chat" button that
 * routed to /chat?project_id=…. Sending a message from the project
 * page:
 *
 *   1. latches the project's id into Stores.Chat.pendingProjectId
 *   2. calls Stores.Chat.sendMessage() — which creates a new
 *      conversation with project_id on the backend (triggering the
 *      MCP-settings snapshot in the same transaction)
 *   3. fires a `conversation.created` event the page listens for and
 *      navigates to /chat/{newConvId}
 *
 * This test pins the navigation contract. The actual LLM context
 * injection is verified separately in
 * `message-uses-project-context.spec.ts` (real-LLM, gated on key).
 */
test.describe('Projects - new conversation via inline ChatInput', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)

    await openCreateProjectDrawer(page)
    await fillProjectForm(page, {
      name: 'Project With Chat',
      instructions: 'Pretend to be a pirate.',
    })
    await submitProjectForm(page)
  })

  test('inline ChatInput is rendered on the project detail page', async ({
    page,
  }) => {
    await page.locator('.ant-card', { hasText: 'Project With Chat' }).click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)

    // ChatInput section is the FIRST section in the new layout.
    await expect(
      page.locator('[data-test-section="chat-input"]'),
    ).toBeVisible()

    // The textarea inside ChatInput should be visible + interactive.
    const textarea = page.locator(
      '[data-test-section="chat-input"] textarea[placeholder*="Type your message"]',
    )
    await expect(textarea).toBeVisible({ timeout: 10000 })
    await expect(textarea).toBeEnabled()

    // No "New chat" navigation button in the header (replaced by the
    // inline ChatInput per Option A).
    await expect(page.getByRole('button', { name: /^new chat$/i })).toHaveCount(0)
  })

  test('sending from inline ChatInput creates conversation with project_id and navigates to /chat/{id}', async ({
    page,
  }) => {
    await page.locator('.ant-card', { hasText: 'Project With Chat' }).click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)
    const projectUrl = page.url()
    const projectId = new URL(projectUrl).pathname.split('/').pop()!

    // Intercept POST /api/conversations so we can assert the project_id
    // body parameter without depending on a real LLM round-trip.
    let capturedProjectId: string | null = null
    page.on('request', async req => {
      if (
        req.method() === 'POST' &&
        req.url().match(/\/api\/conversations(\?|$)/)
      ) {
        try {
          const body = req.postDataJSON?.() ?? JSON.parse(req.postData() || '{}')
          capturedProjectId = body.project_id ?? null
        } catch {
          // ignore body parse failures
        }
      }
    })

    // Type a message in the inline ChatInput. We don't need it to
    // actually reach the LLM — `sendMessage` triggers conversation
    // creation BEFORE the streaming round-trip.
    const textarea = page.locator(
      '[data-test-section="chat-input"] textarea[placeholder*="Type your message"]',
    )
    await expect(textarea).toBeVisible({ timeout: 10000 })
    await textarea.fill('Ahoy there!')

    const sendButton = page.getByRole('button', { name: 'Send message' })
    await expect(sendButton).toBeEnabled({ timeout: 10000 })
    await sendButton.click()

    // The page should navigate to /chat/<convId> shortly after the
    // POST /api/conversations resolves. We allow a generous timeout
    // because the chat extension chain (text + project + assistant +
    // file + mcp) runs synchronously before streaming begins.
    await page.waitForURL(/\/chat\/[0-9a-f-]+/, { timeout: 30000 })

    // The intercepted POST must have carried project_id set to our
    // project's id — that's the contract the snapshot SQL depends on.
    expect(capturedProjectId).toBe(projectId)
  })
})
