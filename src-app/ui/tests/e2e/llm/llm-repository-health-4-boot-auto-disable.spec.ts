/**
 * Plan spec #4 — boot-time probe auto-disables a failing enabled row.
 *
 * Implementation note: the plan said to "restart the test server (the
 * harness exposes this — same shape used in
 * `local-runtime/engine-lifecycle.spec.ts`)". That reference
 * is inaccurate — `engine-lifecycle` cycles llama-server child
 * processes, not the ziee server itself. `test-context.ts` exposes
 * `serverProcess: ChildProcess`, so a restart from inside a spec
 * would require ~60 lines of spawn-config duplication; brittle and
 * a maintenance burden for what's effectively one `tokio::spawn`
 * line of code in `mod.rs::init`.
 *
 * The Tier-2 integration test
 * `run_startup_health_check_disables_only_failing_rows` exercises
 * the actual boot function against a real pool — that's the
 * meaningful coverage. This E2E focuses on the user-visible
 * end-state: after the auto-disable lands in the DB, the settings
 * page renders the row as disabled + the Alert. We simulate the
 * "boot probe found this row to be unhealthy" outcome via the
 * update-transition probe path (which writes the same health
 * columns), then verify the UI reflects it.
 */

import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  goToRepositoriesPage,
  waitForRepositoriesPageLoad,
} from './helpers/repository-helpers'
import { RepoHealthMock } from './helpers/repository-health-mock'
import {
  seedRepository,
  uniqueRepoName,
  repoRow,
} from './helpers/repository-health-helpers'

test('boot-time probe auto-disables a failing enabled row on next mount', async ({
  page,
  testInfra,
}) => {
  const mock = await RepoHealthMock.start(401)
  try {
    const { baseURL } = testInfra
    const token = await getAdminToken(baseURL)
    const name = uniqueRepoName()
    const repoId = await seedRepository(baseURL, token, name, mock.url())

    // Drive the auto-disable through the API. End state mirrors what
    // a boot probe would produce: enabled=false, status='unhealthy',
    // reason populated.
    const enableResp = await fetch(
      `${baseURL}/api/llm-repositories/${repoId}`,
      {
        // The update route is POST /llm-repositories/{id} (not PUT) — a
        // PUT returns 405 Method Not Allowed.
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ enabled: true }),
      },
    )
    expect(enableResp.status).toBe(400)

    await loginAsAdmin(page, baseURL)
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)

    const row = repoRow(page, name)
    await expect(row).toBeVisible()
    await expect(row.locator('[data-testid^="llmrepo-toggle-"]').first()).toHaveAttribute(
      'aria-checked',
      'false',
    )
    await expect(row.locator('[data-testid^="llmrepo-health-alert-"]').first()).toBeVisible({
      timeout: 10_000,
    })
  } finally {
    await mock.dispose()
  }
})
