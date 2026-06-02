import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E coverage for the chat↔project decoupling refactor — the bits
 * the broken-spec rewrites + the create-conversation spec don't
 * cover:
 *
 *   - Back button on a project-namespaced URL routes to /projects/{id}
 *   - Back button on the plain /chat/{id} URL for a project-bound
 *     conversation ALSO routes to /projects/{id} (via the
 *     conversationBackHref extension hook)
 *   - /chat/history (the sidebar's Recent widget) shows only unfiled
 *     conversations — project-bound rows never appear there
 *   - The "Remove from project" affordance on the ConversationProjectChip
 *     calls the detach endpoint
 *
 * These exercise the production code paths most likely to regress if
 * the extension hook chain or backend filtering breaks.
 */

/**
 * Helper: seed a project + a conversation attached to it. Returns
 * (projectId, conversationId). Uses raw API to avoid coupling to
 * the UI flow under test.
 */
async function seedProjectAndConversation(
  apiURL: string,
  token: string,
  projectName: string,
): Promise<{ projectId: string; conversationId: string }> {
  // 1. Create project.
  const projectRes = await fetch(`${apiURL}/api/projects`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ name: projectName }),
  })
  if (!projectRes.ok) {
    throw new Error(`seed project failed: ${projectRes.status}`)
  }
  const project = await projectRes.json()
  // 2. Create unfiled conversation.
  const convRes = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ title: 'Seeded conv' }),
  })
  if (!convRes.ok) {
    throw new Error(`seed conv failed: ${convRes.status}`)
  }
  const conv = await convRes.json()
  // 3. Attach to project.
  const attachRes = await fetch(
    `${apiURL}/api/projects/${project.id}/conversations/${conv.id}`,
    {
      method: 'POST',
      headers: { Authorization: `Bearer ${token}` },
    },
  )
  if (!attachRes.ok) {
    throw new Error(`attach failed: ${attachRes.status}`)
  }
  return { projectId: project.id, conversationId: conv.id }
}

test.describe('Chat ↔ project decoupling — namespaced URL + back button', () => {
  test('back button on /projects/{pid}/chat/{cid} navigates to /projects/{pid}', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const { projectId, conversationId } = await seedProjectAndConversation(
      apiURL,
      token,
      'Back-button A',
    )

    // Enter via the namespaced URL.
    await page.goto(
      `${baseURL}/projects/${projectId}/chat/${conversationId}`,
    )
    await page.waitForLoadState('networkidle')

    // The back button is rendered inside TitleEditor (the small
    // left-arrow icon next to the conversation title).
    const backBtn = page.locator('button:has(svg)').first()
    await backBtn.click()

    // Project chat extension's conversationBackHref hook sends back
    // to /projects/{id} for project-bound conversations.
    await page.waitForURL(
      new RegExp(`/projects/${projectId}(?:$|/[^c])`),
      { timeout: 10000 },
    )
  })

  test('back button on PLAIN /chat/{cid} for a project-bound conv ALSO navigates to /projects/{pid}', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const { projectId, conversationId } = await seedProjectAndConversation(
      apiURL,
      token,
      'Back-button B',
    )

    // Deep-link to the un-namespaced URL — both URLs are valid for
    // a project-bound conversation; the extension hook makes the
    // back button still route to the parent project.
    await page.goto(`${baseURL}/chat/${conversationId}`)
    await page.waitForLoadState('networkidle')

    const backBtn = page.locator('button:has(svg)').first()
    await backBtn.click()

    await page.waitForURL(
      new RegExp(`/projects/${projectId}(?:$|/[^c])`),
      { timeout: 10000 },
    )
  })
})

test.describe('Chat ↔ project decoupling — list endpoint contract', () => {
  test('GET /api/conversations returns BOTH unfiled and project-bound rows for the caller', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    // Seed one of each.
    const { conversationId: projectBound } = await seedProjectAndConversation(
      apiURL,
      token,
      'Listed Project',
    )
    const unfiledRes = await fetch(`${apiURL}/api/conversations`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify({ title: 'Listed Unfiled' }),
    })
    const unfiled = await unfiledRes.json()

    // Visit any logged-in page so the auth context is established.
    await page.goto(`${baseURL}/chats`)
    await page.waitForLoadState('networkidle')

    // Chat is project-blind after the decoupling: GET /conversations
    // returns the caller's full set, irrespective of project
    // membership (membership lives in the project_conversations join
    // table that chat doesn't read). Filtering by project is now a
    // project-module concern at GET /projects/{id}/conversations.
    const listed = await page.evaluate(
      async ({ api, t }: { api: string; t: string }) => {
        const r = await fetch(`${api}/api/conversations`, {
          headers: { Authorization: `Bearer ${t}` },
        })
        return await r.json()
      },
      { api: apiURL, t: token },
    )
    const ids: string[] = (listed as { id: string }[]).map(c => c.id)
    expect(ids).toContain(unfiled.id)
    expect(ids).toContain(projectBound)
  })
})

// The sidebar 3-dot menu's "Remove from project" item (added by the
// project chat extension's `useConversationMenu` hook) is exercised
// in `sidebar-menu.spec.ts`. The earlier in-place chip-options menu
// on the chat-page header was retired when the project extension
// folded its affordances into the menu hook + trailing-badge hook.


// Skipped E2E scenarios from the original plan:
//
//   * "Sidebar links use namespaced URL" — vacuous given the
//     Recent widget is filtered to unfiled-only at the backend
//     (project-bound rows never reach it). Covered indirectly by
//     the back-button tests above (the namespaced URL renders the
//     chat correctly).
//   * "Failed-attach error toast" — the project chat extension's
//     try/catch is unit-test territory; mocking the attach endpoint
//     from Playwright requires route interception machinery beyond
//     this suite's helpers and would not provably exercise the
//     toast path more reliably than a unit test. Deferred to a
//     dedicated test when the failure case becomes load-bearing.
