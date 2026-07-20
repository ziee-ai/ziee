import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'

/**
 * An untitled conversation must be labelled by what the USER actually asked,
 * not by an identical placeholder repeated down the sidebar.
 *
 * Title generation deliberately never persists the user's raw first message as
 * a title (a provider hiccup must not become a permanent bad title), so `title`
 * legitimately stays NULL. Before this change every such row rendered the same
 * "Untitled Conversation" string and the search box matched only the literal
 * word "Untitled" — so the user could neither tell the rows apart nor find one.
 *
 * The label comes from `first_message_preview` on the LIST response, which is
 * why these specs assert after a FULL PAGE RELOAD: the client's per-conversation
 * message cache is empty then, so a fix that only worked for already-opened
 * conversations (the tempting frontend-only approach) would fail here.
 */

const PREVIEW = 'What does the knowledge base say about TP53 mutations'
const REAL_TITLE = 'BRCA1 Role in Hereditary Breast Cancer'
const PLACEHOLDER = 'Untitled Conversation'

/**
 * Create a conversation via the API, optionally titled, optionally with a first
 * user message.
 *
 * `model_id` is REQUIRED by `SendMessageRequest`, so a model must exist even
 * though this test does not care about the assistant's reply — the send
 * persists the USER message (which is all the preview reads) before the
 * provider call, so the provider being a stub that never answers is fine.
 */
async function seedConversation(
  apiURL: string,
  token: string,
  modelId: string,
  opts: { title?: string; firstMessage?: string },
): Promise<string> {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify(opts.title ? { title: opts.title } : {}),
  })
  const conv = await res.json()
  if (opts.firstMessage) {
    // The preview reads the first USER message on the active branch. Seeding it
    // through the normal message path keeps this honest — no direct DB write.
    const sent = await fetch(`${apiURL}/api/conversations/${conv.id}/messages`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({
        content: opts.firstMessage,
        branch_id: conv.active_branch_id,
        model_id: modelId,
      }),
    })
    // Fail loudly here rather than letting a 422 surface later as a confusing
    // "row shows Untitled" assertion failure.
    if (!sent.ok) {
      throw new Error(`seed message failed: ${sent.status} ${await sent.text()}`)
    }
  }
  return conv.id
}

/** Provider + model + group grant, so `model_id` can be supplied on a send. */
async function seedModel(apiURL: string, token: string): Promise<string> {
  const providerId = await createProviderViaAPI(apiURL, token, 'OpenAI', 'openai')
  await assignProviderToAdministratorsGroup(apiURL, token, providerId)
  return await createModelViaAPI(apiURL, token, providerId, undefined, undefined, 'openai')
}

/** The sidebar row for a conversation, by its stable per-id testid. */
function recentRow(page: Page, conversationId: string) {
  return page.getByTestId(`chat-recent-conversations-menu-item-${conversationId}`)
}

test.describe('untitled conversation display label', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  // TEST-12 — the three label states, cold (post-reload).
  test('an untitled conversation is labelled by its first user message', async ({
    page,
    testInfra,
  }) => {
    const { apiURL, baseURL } = testInfra
    const token = await getAdminToken(apiURL)
    const modelId = await seedModel(apiURL, token)

    const untitled = await seedConversation(apiURL, token, modelId, { firstMessage: PREVIEW })
    const titled = await seedConversation(apiURL, token, modelId, {
      title: REAL_TITLE,
      firstMessage: 'some other question entirely',
    })
    const empty = await seedConversation(apiURL, token, modelId, {})

    // FULL reload — the client message cache is cold, so the label can only come
    // from the list response's `first_message_preview`.
    await page.goto(`${baseURL}/chats`)
    await page.reload()

    await expect(recentRow(page, untitled)).toContainText(PREVIEW.slice(0, 40))
    await expect(recentRow(page, untitled)).not.toContainText(PLACEHOLDER)

    // A real title still wins over the preview.
    await expect(recentRow(page, titled)).toContainText(REAL_TITLE)

    // Neither title nor first message → the placeholder is still the last resort.
    await expect(recentRow(page, empty)).toContainText(PLACEHOLDER)
  })

  // TEST-12 (responsive leg) — the derived label can be LONGER than the
  // placeholder it replaces, so the row must still not overflow at mobile width.
  test('the derived label does not overflow the row at 390px', async ({ page, testInfra }) => {
    const { apiURL, baseURL } = testInfra
    const token = await getAdminToken(apiURL)
    const modelId = await seedModel(apiURL, token)
    const long =
      'What does the knowledge base say about the role of TP53 in cell cycle regulation and apoptosis across tumour types'
    const id = await seedConversation(apiURL, token, modelId, { firstMessage: long })

    await page.setViewportSize({ width: 390, height: 844 })
    await page.goto(`${baseURL}/chats`)
    await page.reload()

    const row = recentRow(page, id)
    await expect(row).toBeVisible()

    // The row must clip its label, not widen the page.
    const overflows = await page.evaluate(() => {
      const el = document.scrollingElement!
      return el.scrollWidth > el.clientWidth + 1
    })
    expect(overflows, 'the page must not scroll horizontally at 390px').toBe(false)
  })

  // TEST-13 — search finds it by CONTENT, not by the word "Untitled".
  test('an untitled conversation is findable by its first-message text', async ({
    page,
    testInfra,
  }) => {
    const { apiURL, baseURL } = testInfra
    const token = await getAdminToken(apiURL)
    const modelId = await seedModel(apiURL, token)
    const id = await seedConversation(apiURL, token, modelId, { firstMessage: PREVIEW })

    await page.goto(`${baseURL}/chats`)
    await page.reload()

    const search = page.getByRole('textbox', { name: /search/i }).first()
    await search.fill('TP53')
    await expect(recentRow(page, id)).toBeVisible()

    // And the old escape hatch is gone: a conversation that now renders a real
    // preview must NOT match the placeholder string any more.
    await search.fill('Untitled')
    await expect(recentRow(page, id)).toHaveCount(0)
  })

  // TEST-15 — the display-only contract.
  test('the title editor edits the real title, never the derived preview', async ({
    page,
    testInfra,
  }) => {
    const { apiURL, baseURL } = testInfra
    const token = await getAdminToken(apiURL)
    const modelId = await seedModel(apiURL, token)
    const id = await seedConversation(apiURL, token, modelId, { firstMessage: PREVIEW })

    await page.goto(`${baseURL}/chats/${id}`)

    // The header is the EDIT affordance, so it deliberately shows the honest
    // placeholder rather than a derived label that would imply a title exists.
    await expect(page.getByTestId('conversation-title')).toContainText(PLACEHOLDER)

    await page.getByTestId('chat-title-edit-btn').click()
    const input = page.getByTestId('chat-title-editor-form').getByRole('textbox')

    // The crux: the editor must NOT prefill the preview. Persisting it would
    // re-introduce exactly the raw-message-as-title behavior that was removed.
    await expect(input).toHaveValue('')

    // And the column must still be genuinely NULL on the server.
    const conv = await (
      await fetch(`${apiURL}/api/conversations/${id}`, {
        headers: { Authorization: `Bearer ${token}` },
      })
    ).json()
    expect(conv.title ?? null).toBeNull()
  })
})
