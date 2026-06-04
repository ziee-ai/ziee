import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

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
 * Locate the antd Card on /chats whose title matches. Walks up from
 * the title's `<strong>` to the nearest `.ant-card` ancestor.
 */
function chatsPageCard(
  page: import('@playwright/test').Page,
  title: string,
) {
  return page
    .locator('strong', { hasText: title })
    .locator('xpath=ancestor::*[contains(@class, "ant-card")][1]')
    .first()
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
    await page.waitForLoadState('networkidle')

    // Hover the card to materialize the trailing area (it's mounted
    // lazily on first hover — see ConversationCard's `hoveredOnce`
    // state). The membership lookup then runs and, for an unfiled
    // conversation, renders the "Add to project" button.
    const card = chatsPageCard(page, 'Badge add-to-project conv')
    await expect(card).toBeVisible({ timeout: 10000 })
    await card.hover()

    const addButton = card.getByRole('button', { name: 'Add to project' })
    await expect(addButton).toBeVisible({ timeout: 10000 })
    await addButton.click()

    // Modal opens — scope to the dialog so the menu-item / button
    // text doesn't collide with the modal title.
    const dialog = page.getByRole('dialog', { name: /add to project/i })
    await expect(dialog).toBeVisible({ timeout: 10000 })

    // Filter + Enter — see sidebar-menu.spec.ts for why this is
    // preferred over clicking the option div directly.
    const combo = dialog.getByRole('combobox')
    await combo.click()
    await combo.fill('Badge Add Target')
    await page.keyboard.press('Enter')

    await dialog.getByRole('button', { name: 'Add' }).click()

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
    await page.waitForLoadState('networkidle')

    const card = chatsPageCard(page, 'Badge remove conv')
    await expect(card).toBeVisible({ timeout: 10000 })
    await card.hover()

    // Wait for the trailing area to mount + the project lookup to
    // resolve. Once both are done the Tag with the project name is
    // visible inside the card.
    await expect(card.getByText('Badge Remove Target')).toBeVisible({
      timeout: 10000,
    })

    // antd Tag in v6 renders the close icon as a child `<span>` with
    // `aria-label="close"` (`anticon anticon-close`) — the `.ant-tag-close-icon`
    // class from v5 was dropped. There's exactly one such icon per
    // card (the trailing area's project Tag); the Delete button uses
    // anticon-delete, not anticon-close.
    const closeIcon = card.locator('[aria-label="close"]').first()
    await expect(closeIcon).toBeVisible({ timeout: 10000 })
    await closeIcon.click()

    // Popconfirm bubble appears with title "Remove from project?".
    // The bubble is a body-portal — page-level scoping is correct.
    const popconfirm = page.locator('.ant-popover').filter({
      hasText: 'Remove from project?',
    })
    await expect(popconfirm).toBeVisible({ timeout: 10000 })
    await popconfirm.getByRole('button', { name: 'Remove' }).click()

    await expect(popconfirm).toBeHidden({ timeout: 10000 })
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
