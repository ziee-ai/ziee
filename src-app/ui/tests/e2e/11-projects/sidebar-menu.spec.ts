import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E coverage for `useConversationMenu` — the chat-extension hook
 * that lets the projects module contribute items into the 3-dot
 * dropdown on the sidebar's RecentConversationsWidget. The widget
 * intentionally lives in chat; the menu items are wired in by
 * `projects/chat-extension/extension.tsx`.
 *
 * Selector strategy: navigate to /settings so the main content area
 * doesn't render its own conversation list (only the LeftSidebar's
 * Recent widget shows the rows we created). This avoids strict-mode
 * collisions between the sidebar row and a chats-page card for the
 * same conversation.
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
 * Open the 3-dot menu on the sidebar row whose title matches.
 * Uses the row's aria-labeled title generic as the scoping anchor —
 * the sibling "Conversation options" button is the menu trigger.
 */
async function openSidebarMenuForRow(
  page: import('@playwright/test').Page,
  conversationTitle: string,
) {
  // The row is a `<div>` containing both the aria-labeled title
  // generic AND the "Conversation options" button. Scope to that
  // row first, then click its button. Hover so the trigger renders.
  const titleNode = page.locator(`[title="${conversationTitle}"]`).first()
  await expect(titleNode).toBeVisible({ timeout: 10000 })
  const row = titleNode.locator('xpath=ancestor::div[contains(@class, "group")][1]')
  await row.hover()
  await row.getByRole('button', { name: 'Conversation options' }).click()
}

test.describe('Sidebar conversation menu — project contributions', () => {
  test('"Open: NAME" menu item navigates to the project detail page', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const projectId = await seedProject(apiURL, token, 'Sidebar Open Target')
    const conversationId = await seedUnfiledConv(
      apiURL,
      token,
      'Conv to open project from',
    )
    await attach(apiURL, token, projectId, conversationId)

    // /settings keeps the sidebar visible while the main content
    // doesn't render conversation cards — avoids strict-mode
    // collisions on shared text.
    await page.goto(`${baseURL}/settings`)
    await page.waitForLoadState('networkidle')

    await openSidebarMenuForRow(page, 'Conv to open project from')

    // "Open: NAME" — wait for the menu's portal to render the item.
    await page
      .getByRole('menuitem', { name: /open:\s*sidebar open target/i })
      .click()

    await page.waitForURL(
      new RegExp(`/projects/${projectId}(?:$|/[^c])`),
      { timeout: 10000 },
    )
  })

  test('"Add to project" menu item opens AddToProjectModal and attaches on confirm', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const projectId = await seedProject(apiURL, token, 'Sidebar Add Target')
    const conversationId = await seedUnfiledConv(
      apiURL,
      token,
      'Sidebar add-to-project conv',
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

    await page.goto(`${baseURL}/settings`)
    await page.waitForLoadState('networkidle')

    await openSidebarMenuForRow(page, 'Sidebar add-to-project conv')
    await page.getByRole('menuitem', { name: /add to project/i }).click()

    // Scope to the dialog so we don't collide with the menu item's
    // matching label text. antd Modal renders with role="dialog".
    const dialog = page.getByRole('dialog', { name: /add to project/i })
    await expect(dialog).toBeVisible({ timeout: 10000 })

    // Filter + Enter is more robust than clicking the option element
    // directly: antd's Select option div sometimes fails Playwright's
    // visibility check (the rendered option text varies by version).
    // The combobox uses showSearch + optionFilterProp="label" so
    // typing the project name reliably narrows to one match.
    const combo = dialog.getByRole('combobox')
    await combo.click()
    await combo.fill('Sidebar Add Target')
    await page.keyboard.press('Enter')

    // Confirm via the dialog's Add button.
    await dialog.getByRole('button', { name: 'Add' }).click()

    await expect(dialog).toBeHidden({ timeout: 10000 })
    expect(attachReqUrl).not.toBeNull()

    // Server state: conversation is now in the project.
    const projectQueried = await page.evaluate(
      async ({ api, t, cid }: { api: string; t: string; cid: string }) => {
        const r = await fetch(`${api}/api/projects/by-conversation/${cid}`, {
          headers: { Authorization: `Bearer ${t}` },
        })
        return r.ok ? await r.json() : null
      },
      { api: apiURL, t: token, cid: conversationId },
    )
    expect(projectQueried).not.toBeNull()
    expect(projectQueried.id).toBe(projectId)
  })

  test('"Remove from project" menu item opens Modal.confirm and detaches on OK', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const projectId = await seedProject(apiURL, token, 'Sidebar Remove Target')
    const conversationId = await seedUnfiledConv(
      apiURL,
      token,
      'Sidebar remove conv',
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

    await page.goto(`${baseURL}/settings`)
    await page.waitForLoadState('networkidle')

    await openSidebarMenuForRow(page, 'Sidebar remove conv')
    await page.getByRole('menuitem', { name: /remove from project/i }).click()

    // Modal.confirm renders as a dialog. Scope the title + OK button
    // inside it to avoid colliding with anything else on the page.
    const confirmDialog = page.getByRole('dialog').filter({
      hasText: 'Remove from project?',
    })
    await expect(confirmDialog).toBeVisible({ timeout: 10000 })
    await confirmDialog.getByRole('button', { name: 'Remove' }).click()

    await expect(confirmDialog).toBeHidden({ timeout: 10000 })
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
