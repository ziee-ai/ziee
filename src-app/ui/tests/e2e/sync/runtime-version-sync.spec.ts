import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

// Realtime sync for the `runtime_version` entity (admin local-engine
// versions table). When an admin promotes a version to system-default
// or deletes one, the backend publishes `RuntimeVersion/Update` or
// `/Delete` to every `runtime_versions::read` holder so other devices'
// `useRuntimeVersionStore` refetches without a manual reload.
//
// Why direct SQL seeding (not POST /local-runtime/versions/download):
//
//   The only E2E-reachable creation path is the download endpoint,
//   which fetches binaries from GitHub Releases. That's the same
//   constraint that originally made this test deferred. Rather than
//   either skipping (silent gap) or wiring up a Node port of the mock
//   release server (heavyweight one-off), we use the per-test
//   `testInfra.sql` helper to insert rows directly into the
//   `llm_runtime_versions` table.
//
//   The downside of this shortcut: the rows aren't backed by real
//   binary files in the cache dir, so any code path that tries to
//   spawn the engine would fail. We don't go near that — both the
//   set-default and delete handlers operate purely on the DB row +
//   the sync emit, no binary I/O.
//
//   The set-default handler DOES call BinaryManager::set_system_default,
//   which only touches the DB (flips is_system_default flags); no
//   filesystem check. The delete handler with `remove_binary=false`
//   only deletes the DB row.
//
// Run with --workers=1.

const ENGINE = 'llamacpp'

async function seedRuntimeVersion(
  sql: import('../../fixtures/test-context').TestInfrastructure['sql'],
  version: string,
  isDefault: boolean,
): Promise<string> {
  // Match the schema in migration 21. binary_path doesn't have to point
  // at a real file — neither set_system_default nor delete (without
  // remove_binary) reads it.
  const result = await sql(
    `INSERT INTO llm_runtime_versions
       (engine, version, platform, arch, backend, binary_path, is_system_default)
     VALUES ($1, $2, 'linux', 'x86_64', 'cpu', $3, $4)
     RETURNING id`,
    [
      ENGINE,
      version,
      `/nonexistent/test-only/${version}/llama-server`,
      isDefault,
    ],
  )
  return result.rows[0].id as string
}

async function gotoLocalRuntime(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  await page.goto(`${baseURL}/settings/llm-runtime`)
  await page.waitForLoadState('load')
  // The page renders SettingsPageContainer with title="Local Runtimes",
  // which becomes the stable mount signal. useRuntimeVersionStore
  // subscribes to `sync:runtime_version` in its __init__, fired on
  // first proxy access from the cards below the heading.
  await expect(
    byTestId(page, 'llmrt-runtime-config-card'),
  ).toBeVisible({ timeout: 30_000 })
}

test.describe('Realtime sync — runtime_version (admin engine versions)', () => {
  test('promoting a version to system-default on device A reflects on device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL, sql } = testInfra

    // ── Setup: admin + two seeded versions ──────────────────────────
    // v0.0.98 is the current system-default; v0.0.99 is not. The test
    // promotes v0.0.99 → device B sees the (Default) flag move.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // Two DISTINCT versions — the unique constraint is
    // (engine, version, platform, arch, backend), so seeding the same string
    // twice would 23505. Unique per-run suffix also survives a retry.
    const stamp = Date.now().toString(36)
    const defaultVersion = `0.0.sync-test-default-${stamp}`
    const otherVersion = `0.0.sync-test-other-${stamp}`
    await seedRuntimeVersion(sql, defaultVersion, true)
    const otherId = await seedRuntimeVersion(sql, otherVersion, false)

    await gotoLocalRuntime(page, baseURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoLocalRuntime(pageB, baseURL)

      // Sanity: both rows visible on device B. The card renders
      // `Version <version-string>` as a span.
      await expect(
        byTestId(pageB, `llmrt-version-desc-${defaultVersion}`),
      ).toBeVisible({ timeout: 15_000 })
      await expect(
        byTestId(pageB, `llmrt-version-desc-${otherVersion}`),
      ).toBeVisible({ timeout: 15_000 })

      // Device A promotes otherVersion via REST. The handler calls
      // `BinaryManager::set_system_default` (DB-only — no binary I/O)
      // then `sync_publish(RuntimeVersion, Update, version_id, ...)`.
      const promoteRes = await page.request.post(
        `${baseURL}/api/local-runtime/versions/${otherId}/set-default`,
        { headers: { Authorization: `Bearer ${adminToken}` } },
      )
      expect(
        promoteRes.ok(),
        `set-default failed: ${promoteRes.status()} ${await promoteRes.text()}`,
      ).toBeTruthy()

      // Device B's UI must re-render the (Default) marker on the new
      // default's row AND the "Set as Default" button on the OLD
      // default's row. Each is unique per-version via aria-label /
      // text scoped to the version string — sidestepping the antd
      // card wrapper hierarchy (whose outer divs all contain BOTH
      // version strings + would defeat a plain `hasText` filter).
      //
      // The card hides the Set-as-Default button when
      // `version.is_system_default === true`, so:
      //   - newly-promoted (otherVersion): button DISAPPEARS
      //   - newly-demoted (defaultVersion): button APPEARS
      const promotedSetDefaultBtn = byTestId(pageB, `llmrt-version-set-default-${otherVersion}`)
      const demotedSetDefaultBtn = byTestId(pageB, `llmrt-version-set-default-${defaultVersion}`)

      // After the sync arrives, the promoted row's button is gone
      // (it's now default) and the demoted row's button appears.
      await expect(promotedSetDefaultBtn).toHaveCount(0, { timeout: 20_000 })
      await expect(demotedSetDefaultBtn).toBeVisible({ timeout: 10_000 })
    } finally {
      await ctxB.close()
    }
  })

  test('deleting a version on device A removes it from device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL, sql } = testInfra

    // ── Setup: admin + a non-default seeded version (deletes are
    //    blocked on the system-default, so we delete a non-default
    //    row) ───────────────────────────────────────────────────
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    // Seed a default so a row exists in case of any default-required
    // invariants, then the actual victim row.
    await seedRuntimeVersion(sql, '0.0.sync-keep', true)
    const victimVersion = '0.0.sync-delete'
    const victimId = await seedRuntimeVersion(sql, victimVersion, false)

    await gotoLocalRuntime(page, baseURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoLocalRuntime(pageB, baseURL)

      // Sanity: victim row visible on device B before delete.
      const victimRow = byTestId(pageB, `llmrt-version-desc-${victimVersion}`)
      await expect(victimRow).toBeVisible({ timeout: 15_000 })

      // Device A deletes via REST. `remove_binary=false` (the URL
      // query default) skips the filesystem step, so the synthetic
      // binary_path doesn't matter.
      const deleteRes = await page.request.delete(
        `${baseURL}/api/local-runtime/versions/${victimId}`,
        { headers: { Authorization: `Bearer ${adminToken}` } },
      )
      expect(
        deleteRes.ok(),
        `delete failed: ${deleteRes.status()} ${await deleteRes.text()}`,
      ).toBeTruthy()

      // Device B's UI must drop the row without a reload.
      // Sync emits RuntimeVersion/Delete; useRuntimeVersionStore
      // refetches the list and the missing row disappears.
      await expect(victimRow).toHaveCount(0, { timeout: 20_000 })
    } finally {
      await ctxB.close()
    }
  })
})
