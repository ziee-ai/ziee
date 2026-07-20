import { test, expect } from '../../fixtures/test-context'
import { getCurrentUserToken } from '../../common/auth-helpers'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/permissions'
import { byTestId } from '../testid'

// TEST-49 (ITEM-34, restricted-user / A10): a user LACKING knowledge_base::use
// must see NO Knowledge Base UI, across all four gating layers —
//   slot   → the "Knowledge" sidebar nav entry is absent,
//   route  → a direct URL to /knowledge does NOT render the KB list page,
//   <Can>  → the create-KB / add-doc affordances are absent (unreachable — the
//            route is gated — and gated by <Can KnowledgeBaseManage> regardless),
//   usePermission → the KB project knowledge-kind (inline preview + manage
//            panel) is absent and fires no 403.
//
// The user MUST be isolated from the default group, because migration 147 grants
// knowledge_base::use to the default Users group — `loginWithPerms` removes the
// new user from it and grants only the listed direct perms. Positive control =
// TEST-41 (a permitted user DOES see the KB panel on the same project surface).
test.describe('Knowledge Base — permission gating (no knowledge_base::use)', () => {
  test('an unpermitted user sees no KB UI (nav + route + project knowledge-kind)', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // A user who can READ/CREATE/EDIT projects (so the app shell is populated
    // and they can own + open a project) but who, isolated from the default
    // group, genuinely LACKS knowledge_base::use.
    await loginWithPerms(page, baseURL, apiURL, [
      Permissions.ProjectsRead,
      Permissions.ProjectsCreate,
      Permissions.ProjectsEdit,
    ])
    const token = await getCurrentUserToken(page)
    expect(token, 'restricted user should be logged in').toBeTruthy()

    // Capture any 4xx on a KB endpoint — an ungated component would call the KB
    // API and 403 for this user.
    const kb4xx: string[] = []
    page.on('response', r => {
      if (r.url().includes('/knowledge-bases') && r.status() >= 400)
        kb4xx.push(`${r.status()} ${r.url()}`)
    })

    // ── Layer 1 (slot): no "Knowledge" navigation entry anywhere in the shell.
    await page.goto(`${baseURL}/`)
    await expect(page.getByRole('link', { name: 'Knowledge', exact: true })).toHaveCount(0)
    await expect(page.getByRole('menuitem', { name: 'Knowledge', exact: true })).toHaveCount(0)

    // ── Layer 2 (route): a direct URL to /knowledge must NOT render the KB list
    // page (the route is permission-gated → denied/redirect, not the page).
    await page.goto(`${baseURL}/knowledge`)
    await expect(byTestId(page, 'kb-list-title')).toHaveCount(0)
    await expect(byTestId(page, 'kb-list-create-button')).toHaveCount(0)
    await expect(byTestId(page, 'kb-list-empty-create-button')).toHaveCount(0)

    // ── Layer 4 (usePermission): the KB project knowledge-kind is absent.
    const proj = await page.request.post(`${apiURL}/api/projects`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { name: `NoKB Project ${Date.now().toString(36)}` },
    })
    const projectId: string = (await proj.json()).id
    await page.goto(`${baseURL}/projects/${projectId}`)

    // The project's own "Project knowledge" section still renders (the file
    // knowledge-kind is not KB-gated)…
    await expect(byTestId(page, 'project-knowledge-manage-button')).toBeVisible()
    // …but the KB inline preview is entirely absent (no count link a permitted
    // user would see on a fresh project).
    await expect(byTestId(page, 'kb-project-inline-manage-link')).toHaveCount(0)

    // The manage drawer opens; the KB manage panel — attach button, empty state,
    // and count tag, all of which a permitted user always sees here — is absent.
    await byTestId(page, 'project-knowledge-manage-button').click()
    const drawer = page.getByRole('dialog')
    await expect(drawer).toBeVisible()
    await expect(byTestId(drawer, 'kb-project-attach-button')).toHaveCount(0)
    await expect(byTestId(drawer, 'kb-project-panel-empty')).toHaveCount(0)
    await expect(byTestId(drawer, 'kb-project-panel-count-tag')).toHaveCount(0)

    // No KB endpoint was ever called for this unpermitted user (no gated
    // component fetched → no 403).
    expect(kb4xx, 'no KB endpoint should be called for an unpermitted user').toEqual([])
  })
})
