import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

/**
 * E2E coverage for the project chat extension's
 * `renderConversationCardTrailing` hook — the hover-mounted badge on
 * ConversationCard rendered on the /chats history page. Two states:
 *
 *   - Unfiled conversation → "Add to project" button → opens
 *     AddToProjectModal → pick + Add fires the attach POST.
 *   - Project-bound conversation → "In project: NAME" Tag with a
 *     close (×) icon → opens a Popconfirm anchored to the tag → OK
 *     fires the detach DELETE.
 *
 * Selector strategy: the /chats history list renders each
 * conversation as an antd Card with the title in a `<strong>` tag.
 * Hovering the Card mounts the trailing area (which is otherwise
 * lazy — see ConversationCard's `hoveredOnce` state). All locators
 * scope under the matching card to avoid colliding with the
 * sidebar's Recent widget that ALSO shows the same titles.
 */

async function seedProject(
  apiURL: string,
  token: string,
  name: string,
): Promise<string> {
  const res = await fetch(`${apiURL}/api/projects`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ name }),
  })
  if (!res.ok) throw new Error(`seed project failed: ${res.status}`)
  return (await res.json()).id
}

async function seedUnfiledConv(
  apiURL: string,
  token: string,
  title: string,
): Promise<string> {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) throw new Error(`seed conv failed: ${res.status}`)
  return (await res.json()).id
}

async function attach(
  apiURL: string,
  token: string,
  projectId: string,
  conversationId: string,
) {
  const res = await fetch(
    `${apiURL}/api/projects/${projectId}/conversations/${conversationId}`,
    { method: 'POST', headers: { Authorization: `Bearer ${token}` } },
  )
  if (!res.ok) throw new Error(`attach failed: ${res.status}`)
}

/**
 * Locate the ConversationCard on /chats by conversation id (stable
 * `chat-conversation-card-<id>` testid).
 */
function chatsPageCard(
  page: import('@playwright/test').Page,
  conversationId: string,
) {
  return byTestId(page, `chat-conversation-card-${conversationId}`)
}

test.describe('ConversationCard trailing badge — add-to-project', () => {
  test('hover → "Add to project" → modal → pick → Add fires attach', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const projectId = await seedProject(apiURL, token, 'Badge Add Target')
    const conversationId = await seedUnfiledConv(
      apiURL,
      token,
      'Badge add-to-project conv',
    )

    let attachReqUrl: string | null = null
    page.on('request', req => {
      if (
        req.method() === 'POST' &&
        req.url().includes(
          `/api/projects/${projectId}/conversations/${conversationId}`,
        )
      ) {
        attachReqUrl = req.url()
      }
    })

    await page.goto(`${baseURL}/chats`)
    await page.waitForLoadState('load')

    // Hover the card to materialize the trailing area (it's mounted
    // lazily on first hover — see ConversationCard's `hoveredOnce`
    // state). The membership lookup then runs and, for an unfiled
    // conversation, renders the "Add to project" button.
    const card = chatsPageCard(page, conversationId)
    await expect(card).toBeVisible({ timeout: 10000 })
    await card.hover()

    const addButton = byTestId(card, 'project-trailing-add-button')
    await expect(addButton).toBeVisible({ timeout: 10000 })
    await addButton.click()

    // Modal opens.
    const dialog = byTestId(page, 'project-add-to-project-dialog')
    await expect(dialog).toBeVisible({ timeout: 10000 })

    // Open the combobox + pick the project by its derived option testid.
    // Open the combobox and dispatch the click straight to the option: the
    // Base-UI list virtualizes + the option's hover transition keeps it "not
    // stable", so the actionability path never fires the selecting onClick.
    await byTestId(dialog, 'project-add-to-project-combobox').click()
    await byTestId(
      page,
      `project-add-to-project-combobox-opt-${projectId}`,
    ).dispatchEvent('click')

    await byTestId(dialog, 'project-add-to-project-confirm-button').click()

    await expect(dialog).toBeHidden({ timeout: 10000 })
    expect(attachReqUrl).not.toBeNull()

    const project = await page.evaluate(
      async ({ api, t, cid }: { api: string; t: string; cid: string }) => {
        const r = await fetch(`${api}/api/projects/by-conversation/${cid}`, {
          headers: { Authorization: `Bearer ${t}` },
        })
        return r.ok ? await r.json() : null
      },
      { api: apiURL, t: token, cid: conversationId },
    )
    expect(project?.id).toBe(projectId)
  })
})

test.describe('ConversationCard trailing badge — remove-from-project', () => {
  test('hover → "In project: NAME" tag → × → popconfirm → Remove fires detach', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const projectId = await seedProject(apiURL, token, 'Badge Remove Target')
    const conversationId = await seedUnfiledConv(
      apiURL,
      token,
      'Badge remove conv',
    )
    await attach(apiURL, token, projectId, conversationId)

    let detachSeen = false
    page.on('request', req => {
      if (
        req.method() === 'DELETE' &&
        req.url().includes(
          `/api/projects/${projectId}/conversations/${conversationId}`,
        )
      ) {
        detachSeen = true
      }
    })

    await page.goto(`${baseURL}/chats`)
    await page.waitForLoadState('load')

    const card = chatsPageCard(page, conversationId)
    await expect(card).toBeVisible({ timeout: 10000 })
    await card.hover()

    // Wait for the trailing area to mount + the project lookup to resolve.
    // For a project-bound conversation the "Remove from project" button
    // renders inside the card (it replaced the old membership Tag + × icon).
    const removeButton = byTestId(card, 'project-trailing-remove-button')
    await expect(removeButton).toBeVisible({ timeout: 10000 })
    await removeButton.click()

    // The remove Confirm (AlertDialog) opens; confirm via its primary
    // button (`<confirm-testid>-confirm`).
    const confirm = byTestId(page, 'project-trailing-remove-confirm')
    await expect(confirm).toBeVisible({ timeout: 10000 })
    await byTestId(page, 'project-trailing-remove-confirm-confirm').click()

    await expect(confirm).toBeHidden({ timeout: 10000 })
    expect(detachSeen).toBe(true)

    // `/api/projects/by-conversation/{cid}` always returns 200 with
    // null body when unfiled — see the matching note in
    // delete-project-leaves-orphan-conversations.spec.ts and
    // server/.../chat_extension/handlers.rs::project_for_conversation.
    const lookup = await page.evaluate(
      async ({ api, t, cid }: { api: string; t: string; cid: string }) => {
        const r = await fetch(`${api}/api/projects/by-conversation/${cid}`, {
          headers: { Authorization: `Bearer ${t}` },
        })
        const body = r.ok ? await r.json() : 'error'
        return { status: r.status, body }
      },
      { api: apiURL, t: token, cid: conversationId },
    )
    expect(lookup.status).toBe(200)
    expect(lookup.body).toBeNull()
  })
})
