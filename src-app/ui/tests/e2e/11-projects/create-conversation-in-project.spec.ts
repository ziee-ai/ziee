import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  fillProjectForm,
  getProjectCard,
  goToProjectsPage,
  openCreateProjectDrawer,
  submitProjectForm,
} from './helpers/project-helpers'

/**
 * Project detail page hosts an inline `<ChatInput>`. After the
 * chat↔project decoupling refactor, sending from this input runs
 * the two-call flow:
 *
 *   1. POST /api/conversations               (chat creates UNFILED)
 *   2. POST /api/projects/{pid}/conversations/{cid}  (attach)
 *
 * The attach call is fired by the frontend project chat extension's
 * `afterCreateConversation` hook (chat's `Stores.Chat.sendMessage`
 * chains it before emitting `conversation.created`).
 *
 * `ProjectDetailPage` subscribes to `conversation.created` and
 * navigates to the project-namespaced URL
 * `/projects/{pid}/chat/{cid}` — both URLs (`/chat/{id}` and the
 * namespaced form) remain valid for a project-bound conversation,
 * but the project page resolves to the canonical namespaced form.
 *
 * This test pins the navigation + API-call contract. The actual
 * LLM context injection is verified separately in
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
    await getProjectCard(page, 'Project With Chat').click()
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
    await expect(
      byTestId(page, 'project-detail-new-chat-button'),
    ).toHaveCount(0)
  })

  test('sending from inline ChatInput fires create + attach and navigates to /projects/{pid}/chat/{cid}', async ({
    page,
  }) => {
    await getProjectCard(page, 'Project With Chat').click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)
    const projectUrl = page.url()
    const projectId = new URL(projectUrl).pathname.split('/').pop()!

    // Intercept both API calls so we can assert the two-step flow.
    //   - POST /api/conversations: chat's default create (UNFILED;
    //     body must NOT include project_id post-decoupling).
    //   - POST /api/projects/{pid}/conversations/{cid}: the project
    //     chat extension's attach call.
    let createBodyHasProjectId: boolean | null = null
    let attachUrlSeen: string | null = null
    page.on('request', req => {
      const url = req.url()
      if (
        req.method() === 'POST' &&
        url.match(/\/api\/conversations(\?|$)/)
      ) {
        try {
          const body = req.postDataJSON?.() ?? JSON.parse(req.postData() || '{}')
          createBodyHasProjectId = Object.prototype.hasOwnProperty.call(
            body,
            'project_id',
          )
        } catch {
          // ignore body parse failures
        }
      } else if (
        req.method() === 'POST' &&
        url.match(
          new RegExp(`/api/projects/${projectId}/conversations/[0-9a-f-]+`),
        )
      ) {
        attachUrlSeen = url
      }
    })

    // Type a message in the inline ChatInput. We don't need it to
    // actually reach the LLM — `sendMessage` chains create + attach
    // BEFORE the streaming round-trip.
    const textarea = page.locator(
      '[data-test-section="chat-input"] textarea[placeholder*="Type your message"]',
    )
    await expect(textarea).toBeVisible({ timeout: 10000 })
    await textarea.fill('Ahoy there!')

    const sendButton = byTestId(page, 'chat-input-send-btn')
    await expect(sendButton).toBeEnabled({ timeout: 10000 })
    await sendButton.click()

    // Page navigates to the project-NAMESPACED URL once the attach
    // call resolves and `conversation.created` fires with the
    // post-hook conversation shape.
    await page.waitForURL(
      new RegExp(`/projects/${projectId}/chat/[0-9a-f-]+`),
      { timeout: 30000 },
    )

    // Contract: chat's create no longer carries project_id (the
    // decoupling moved this concern to the project module).
    expect(createBodyHasProjectId).toBe(false)
    // Contract: the attach endpoint was hit for this project.
    expect(attachUrlSeen).not.toBeNull()
  })
})
