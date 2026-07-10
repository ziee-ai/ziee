import { test, expect } from '../../fixtures/test-context'
import { getCurrentUserToken } from '../../common/auth-helpers'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/types'
import { byTestId } from '../testid'

// TEST-49 (ITEM-34 gating hardening): permission gating of the KB project
// knowledge-kind. A user WITHOUT `knowledge_base::use` can still open a project
// they own, but must see NO trace of the Knowledge Base feature there — the
// inline preview and the manage-drawer panel are gated (return null) and must
// not even fire the `listProject` fetch (which 403s + toasts for such a user).
//
// NOTE: migration 134 grants knowledge_base::use to the default Users group, so
// the user MUST be isolated from that group to genuinely lack it — that's what
// `loginWithPerms` does (it removes the new user from the default group and
// grants only the listed direct perms). Positive control = TEST-41 (an admin /
// permitted user DOES see the KB panel on the same surface).
test.describe('Knowledge Base — permission gating (no knowledge_base::use)', () => {
  test('an unpermitted user sees no KB knowledge-kind in a project', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    // A user who can READ/CREATE/EDIT projects (so they can own + open one —
    // projects are owner-scoped) but who, isolated from the default group,
    // genuinely LACKS knowledge_base::use.
    await loginWithPerms(page, baseURL, apiURL, [
      Permissions.ProjectsRead,
      Permissions.ProjectsCreate,
      Permissions.ProjectsEdit,
    ])
    const token = await getCurrentUserToken(page)
    expect(token, 'restricted user should be logged in').toBeTruthy()

    // The user owns a fresh project (no KBs attached).
    const proj = await page.request.post(`${apiURL}/api/projects`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { name: `NoKB Project ${Date.now().toString(36)}` },
    })
    const projectId: string = (await proj.json()).id

    // An ungated component would call GET /projects/{id}/knowledge-bases and get
    // a 403 for this user — capture any 4xx on a KB endpoint.
    const kb4xx: string[] = []
    page.on('response', r => {
      if (r.url().includes('/knowledge-bases') && r.status() >= 400)
        kb4xx.push(`${r.status()} ${r.url()}`)
    })

    await page.goto(`${baseURL}/projects/${projectId}`)

    // The project's own "Project knowledge" section still renders (the file
    // knowledge-kind is not KB-gated)…
    await expect(byTestId(page, 'project-knowledge-manage-button')).toBeVisible()

    // …but the KB inline preview is entirely absent (no count link, which a
    // permitted user WOULD see on a fresh project).
    await expect(byTestId(page, 'kb-project-inline-manage-link')).toHaveCount(0)

    // The manage drawer opens, and the KB manage panel — attach button, empty
    // state, and count tag, all of which a permitted user always sees here — is
    // absent entirely.
    await byTestId(page, 'project-knowledge-manage-button').click()
    const drawer = page.getByRole('dialog')
    await expect(drawer).toBeVisible()
    await expect(byTestId(drawer, 'kb-project-attach-button')).toHaveCount(0)
    await expect(byTestId(drawer, 'kb-project-panel-empty')).toHaveCount(0)
    await expect(byTestId(drawer, 'kb-project-panel-count-tag')).toHaveCount(0)

    // The gated components never fetched, so no KB endpoint 4xx (403) fired.
    expect(kb4xx, 'no KB endpoint should be called for an unpermitted user').toEqual([])
  })
})
