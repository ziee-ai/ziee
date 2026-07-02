import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

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
 * Open the 3-dot menu on the sidebar row for the given conversation.
 * The RecentConversationsWidget renders a per-row actions button with a
 * stable `chat-recent-row-actions-btn-<id>` testid (hover-revealed via
 * opacity — still clickable + visible to Playwright).
 */
async function openSidebarMenuForRow(
  page: import('@playwright/test').Page,
  conversationId: string,
) {
  const trigger = byTestId(page, `chat-recent-row-actions-btn-${conversationId}`)
  await expect(trigger).toBeVisible({ timeout: 10000 })
  await trigger.hover()
  await trigger.click()
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
    await page.waitForLoadState('load')

    await openSidebarMenuForRow(page, conversationId)

    // "Open: NAME" menu item (derived id from its `project-open` key).
    await byTestId(
      page,
      `chat-recent-row-menu-${conversationId}-item-project-open`,
    ).click()

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

    // Land on /chats: the sidebar (recent-conversations widget + its row menu) is
    // global, and this route pre-loads the projects store so the AddToProject
    // combobox is populated + actionable (on /settings it lazy-loads on modal
    // open, leaving the combobox briefly non-interactive).
    await page.goto(`${baseURL}/chats`)
    await page.waitForLoadState('load')

    await openSidebarMenuForRow(page, conversationId)
    await byTestId(
      page,
      `chat-recent-row-menu-${conversationId}-item-project-add`,
    ).click()

    const dialog = byTestId(page, 'project-add-to-project-dialog')
    await expect(dialog).toBeVisible({ timeout: 10000 })

    // Open the combobox + pick the project by its derived option testid.
    // Open the combobox, type to filter so the target option renders (the list
    // virtualizes — an off-screen option never attaches), then dispatch the
    // click straight to it (its hover transition keeps it "not stable", so the
    // actionability path never fires the selecting onClick).
    // Open the combobox and dispatch the click straight to the option: the
    // Base-UI list virtualizes + the option's hover transition keeps it "not
    // stable", so the actionability path never fires the selecting onClick.
    await byTestId(dialog, 'project-add-to-project-combobox').click()
    await byTestId(
      page,
      `project-add-to-project-combobox-opt-${projectId}`,
    ).dispatchEvent('click')

    // Confirm via the dialog's Add button.
    await byTestId(dialog, 'project-add-to-project-confirm-button').click()

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
    await page.waitForLoadState('load')

    await openSidebarMenuForRow(page, conversationId)
    await byTestId(
      page,
      `chat-recent-row-menu-${conversationId}-item-project-remove`,
    ).click()

    // dialog.confirm renders a Radix AlertDialog (role="alertdialog").
    // Its primary (Remove) action is the last footer button.
    const confirmDialog = page.getByRole('alertdialog')
    await expect(confirmDialog).toBeVisible({ timeout: 10000 })
    await confirmDialog.getByRole('button').last().click()

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
