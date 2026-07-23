import { test, expect } from '../../fixtures/test-context'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/permissions'
import { byTestId } from '../testid'

// A10 [negative-perm]: the workflow BUILDER is a NEW user-facing surface gated on
// the (existing) workflows::install (create) + workflows::manage (edit) perms.
// Reusing an existing permission for a new surface still requires the
// frontend-HIDDEN proof — a user LACKING those perms must see NO builder UI, at
// every layer: the create affordance (<Can> / slot) AND the builder + edit routes
// (route permission gate → inline 403), not merely a 403 on save.
test.describe('Workflows — builder gating (restricted user)', () => {
  test('a user lacking workflows::install/manage sees NO builder UI', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    // WorkflowsRead + WorkflowsExecute = can VIEW + RUN workflows, but holds
    // NEITHER workflows::install NOR workflows::manage (create/edit).
    await loginWithPerms(page, baseURL, apiURL, [
      Permissions.WorkflowsRead,
      Permissions.WorkflowsExecute,
    ])

    // The workflows list itself renders (read perm) …
    await page.goto(`${baseURL}/settings/workflows`)
    await expect(byTestId(page, 'wf-list-page-title')).toBeVisible()
    // … but BOTH create affordances are absent (<Can workflows::install>):
    // the "New workflow" (builder) button and the Import button.
    await expect(byTestId(page, 'wf-list-new-btn')).toHaveCount(0)
    await expect(byTestId(page, 'wf-list-import-btn')).toHaveCount(0)

    // The builder CREATE route (workflows::install) is blocked: the restricted
    // user is redirected away by the route gate (and the page's own permission
    // guard renders a 403 result if it ever mounts) — either way the builder
    // surface is ABSENT.
    await page.goto(`${baseURL}/settings/workflows/builder`)
    await expect(byTestId(page, 'wf-builder-page-title')).toHaveCount(0)
    await expect(byTestId(page, 'wf-builder-add-step-btn')).toHaveCount(0)

    // The EDIT route (workflows::manage) is blocked for ANY id — before the
    // workflow is ever loaded — so the builder surface is likewise ABSENT.
    await page.goto(
      `${baseURL}/settings/workflows/00000000-0000-0000-0000-000000000000/edit`,
    )
    await expect(byTestId(page, 'wf-builder-page-title')).toHaveCount(0)
  })
})
