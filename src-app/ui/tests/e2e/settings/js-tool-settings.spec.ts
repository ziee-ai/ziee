import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/permissions'
import { byTestId } from '../testid'

/**
 * E2E — the admin "Programmatic Tools" settings page (/settings/js-tool): the
 * runtime resource caps for the built-in run_js tool. Round-trips through
 * GET/PUT /api/js-tool/settings; the InputNumber fields clamp out-of-range
 * values on blur (the footgun guard); a change syncs cross-window; and a
 * non-admin is blocked.
 */
async function gotoJsTool(page: import('@playwright/test').Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/js-tool`, { waitUntil: 'domcontentloaded' })
  await expect(byTestId(page, 'js-tool-settings-card')).toBeVisible({ timeout: 30_000 })
}

test.describe('Programmatic Tools — run_js limits admin page', () => {
  // TEST-50: admin edits a cap + it persists across a reload (GET/PUT round-trip).
  test('admin edits + persists a run_js limit', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await gotoJsTool(page, baseURL)

    // Default wall-clock is 300s.
    const wall = byTestId(page, 'js-tool-wall')
    await expect(wall).toHaveValue(/300/)

    await wall.fill('120')
    await wall.blur()
    const save = byTestId(page, 'js-tool-settings-save-btn')
    await expect(save).toBeEnabled()
    // Wait for the save PUT to COMPLETE before reloading — otherwise the reload
    // can cancel the in-flight request and the persistence assertion races.
    const savePut = page.waitForResponse(
      r => r.url().includes('/api/js-tool/settings') && r.request().method() === 'PUT',
    )
    await save.click()
    const putResp = await savePut
    expect(putResp.ok()).toBeTruthy()

    // Persisted server-side: a hard reload re-fetches the row.
    await page.reload({ waitUntil: 'domcontentloaded' })
    await expect(byTestId(page, 'js-tool-settings-card')).toBeVisible({ timeout: 30_000 })
    await expect(byTestId(page, 'js-tool-wall')).toHaveValue(/120/, { timeout: 15_000 })
  })

  // TEST-51: an out-of-range value can't be submitted — the InputNumber clamps
  // to the bound on blur (validation-rejects-absurd, visible in the UI). Also
  // confirms the server rejects an absurd direct PUT with 422.
  test('rejects absurd values (clamp + server 422)', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await gotoJsTool(page, baseURL)

    const runs = byTestId(page, 'js-tool-max-runs')
    await runs.fill('999') // > max 256
    await runs.blur()
    await expect(runs).toHaveValue(/256/) // clamped — can't submit an absurd cap

    // The server is the last line of defense: a direct absurd PUT is a 422.
    const token = await getAdminToken(apiURL)
    const res = await page.request.put(`${apiURL}/api/js-tool/settings`, {
      headers: { Authorization: `Bearer ${token}` },
      data: { memory_bytes: 1 },
    })
    expect(res.status()).toBe(422)
  })

  // TEST-53: a non-admin lacking js_tool::settings::read is blocked from the page.
  test('non-admin without js_tool::settings::read is blocked', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    // A user with an unrelated read perm — enough to log in, but lacking
    // js_tool::settings::read which gates the /settings/js-tool route.
    await loginWithPerms(page, baseURL, apiURL, [Permissions.SkillsRead])
    await page.goto(`${baseURL}/settings/js-tool`, { waitUntil: 'domcontentloaded' })
    await expect(byTestId(page, 'router-route-forbidden-result')).toBeVisible()
    await expect(byTestId(page, 'js-tool-settings-card')).toHaveCount(0)
  })
})

test.describe('Realtime sync — run_js limits (cross-window)', () => {
  // TEST-52: a change on device A reflects on device B live (sync:js_tool_settings).
  test('changing max_concurrent_runs on device A reflects on device B live', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await gotoJsTool(page, baseURL)
    const token = await getAdminToken(apiURL)

    const cur = await (
      await fetch(`${apiURL}/api/js-tool/settings`, {
        headers: { Authorization: `Bearer ${token}` },
      })
    ).json()
    const next = cur.max_concurrent_runs === 16 ? 12 : 16

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoJsTool(pageB, baseURL)
      await expect(byTestId(pageB, 'js-tool-max-runs')).toHaveValue(
        new RegExp(String(cur.max_concurrent_runs)),
        { timeout: 15_000 },
      )

      // Change it on device A via the REST API.
      const put = await fetch(`${apiURL}/api/js-tool/settings`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify({ max_concurrent_runs: next }),
      })
      expect(put.ok).toBeTruthy()

      // Device B's field updates live via sync→reload→re-seed.
      await expect(byTestId(pageB, 'js-tool-max-runs')).toHaveValue(
        new RegExp(String(next)),
        { timeout: 15_000 },
      )
    } finally {
      await ctxB.close()
    }
  })
})
