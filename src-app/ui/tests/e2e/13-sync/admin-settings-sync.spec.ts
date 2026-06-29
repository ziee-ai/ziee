import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import {
  loginAsAdmin,
  getCurrentUserToken,
} from '../../common/auth-helpers'

// Realtime sync for THREE permission-scoped admin/deployment entities. Unlike
// the owner-scoped assistant/project specs (which prove per-user isolation),
// these entities are admin-only singletons or admin-curated lists whose sync
// audience is "everyone holding the matching read permission":
//
//   - llm_repository        → sync:llm_repository        (LlmRepositoriesRead)
//   - runtime_settings      → sync:runtime_settings      (RuntimeSettingsRead)
//   - code_sandbox_settings → sync:code_sandbox_settings (CodeSandboxResourceLimitsRead)
//
// Shape of every test: the SAME admin user on two browser contexts (device A
// and device B). A mutates (via its own page-session token so the change is
// attributable to A's session) and B must reflect the change live — no manual
// reload — because the server pushes the sync event and B's store refetches.
//
// CRITICAL: this suite NEVER calls waitForLoadState('networkidle'). The
// realtime-sync SSE stream is a persistent connection that keeps the network
// perpetually busy, so 'networkidle' never settles and would hang the whole
// test. We navigate inline and wait on a STABLE selector instead, keeping the
// cross-device flow self-contained rather than reusing the 05-llm repository
// nav/create helpers.
//
// Run with --workers=1 (shared backend + DB).

// ── inline, networkidle-free navigation ─────────────────────────────────────

async function gotoRepositories(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  await page.goto(`${baseURL}/settings/llm-repositories`)
  await page.waitForLoadState('load')
  // The settings card heading is the stable "page loaded" signal.
  await expect(
    byTestId(page, 'llmrepo-card'),
  ).toBeVisible({ timeout: 30_000 })
}

async function gotoRuntimeSettings(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  await page.goto(`${baseURL}/settings/llm-runtime`)
  await page.waitForLoadState('load')
  // The Runtime configuration card is the stable signal that the singleton
  // settings store has mounted (and subscribed to sync:runtime_settings).
  await expect(
    byTestId(page, 'llmrt-runtime-config-card'),
  ).toBeVisible({ timeout: 30_000 })
}

async function gotoSandboxSettings(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  await page.goto(`${baseURL}/settings/sandbox`)
  await page.waitForLoadState('load')
  // The combined "Code Sandbox" page heading. The route is admitted by
  // code_sandbox::resource_limits::read and is reachable even when the
  // sandbox runtime is DISABLED — the resource-limits row is a plain DB
  // singleton (GET/PUT /api/code-sandbox/resource-limits), independent of
  // whether bwrap/squashfuse is wired up.
  await expect(
    byTestId(page, 'sandbox-resource-limits-card'),
  ).toBeVisible({ timeout: 30_000 })
}

// ── REST mutation helpers (device A drives them with ITS OWN session token) ──
// Mutating through A's page-session token makes the change attributable to
// device A — the SyncOrigin carried on the request lets the backend route the
// resulting sync event to all OTHER sessions of the same audience (i.e. B),
// exactly the cross-device path we want to prove.

async function jsonHeaders(page: import('@playwright/test').Page) {
  const token = await getCurrentUserToken(page)
  return { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }
}

// ── tests ───────────────────────────────────────────────────────────────────

test.describe('Realtime sync — admin settings (permission-scoped)', () => {
  test('an LLM repository created on device A appears on the same admin device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Device A — fresh per-test backend; loginAsAdmin onboards the admin.
    await loginAsAdmin(page, baseURL)
    await gotoRepositories(page, baseURL)

    // Device B — a second context for the SAME admin user. Load it fully
    // before A mutates so its repositories store is mounted + subscribed to
    // sync:llm_repository when the event fires.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoRepositories(pageB, baseURL)

      const name = `Sync Repo ${Date.now()}`

      // Mutate from device A's session (its own bearer token). We create via
      // the REST endpoint rather than the drawer UI — the sync event is emitted
      // by the backend regardless of whether the mutation came from UI or API.
      const res = await page.request.post(
        `${baseURL}/api/llm-repositories`,
        {
          headers: await jsonHeaders(page),
          data: { name, url: 'https://example.com/sync', auth_type: 'none', enabled: true },
        },
      )
      expect(res.ok()).toBeTruthy()

      // Device B must show the new repository row WITHOUT a manual reload —
      // the SSE sync:llm_repository event makes B's store refetch the list.
      await expect(byTestId(pageB, 'llmrepo-card')).toContainText(name, {
        timeout: 15_000,
      })
    } finally {
      await ctxB.close()
    }
  })

  test('a runtime-settings change on device A is reflected in admin device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Device A.
    await loginAsAdmin(page, baseURL)
    await gotoRuntimeSettings(page, baseURL)

    // Device B — same admin, second context, fully loaded first so its
    // RuntimeConfig store is subscribed to sync:runtime_settings.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoRuntimeSettings(pageB, baseURL)

      // The first spinbutton in the Runtime configuration card is
      // idle_unload_secs ("Idle unload timeout (seconds)"). Read B's current
      // value so we pick a NEW value distinct from it (avoids a no-op assert).
      const configCardB = byTestId(pageB, 'llmrt-runtime-config-card')
      const idleInputB = configCardB.getByTestId('llmrt-config-idle-unload')
      await expect(idleInputB).toBeVisible({ timeout: 15_000 })
      const beforeValB = (await idleInputB.inputValue()).trim()

      // Choose a new idle_unload_secs value that differs from B's current one
      // (valid range is 0..=86400).
      const newIdle = beforeValB === '123' ? 234 : 123

      // Mutate the singleton from device A's session via the REST endpoint.
      const res = await page.request.put(
        `${baseURL}/api/local-runtime/settings`,
        {
          headers: await jsonHeaders(page),
          data: { idle_unload_secs: newIdle },
        },
      )
      expect(res.ok()).toBeTruthy()

      // Device B's control must show the NEW field value without a reload —
      // sync:runtime_settings triggers B's store to refetch + the card's
      // useEffect re-seeds the form from the new singleton.
      await expect(idleInputB).toHaveValue(String(newIdle), { timeout: 15_000 })
    } finally {
      await ctxB.close()
    }
  })

  test('a sandbox resource-limit change on device A is reflected in admin device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Device A. The /settings/sandbox route + the resource-limits REST endpoint
    // are reachable even when the code_sandbox RUNTIME is disabled in the test
    // config (the limits row is a plain DB singleton). If a future config
    // change makes the page unreachable, the gotoSandboxSettings heading wait
    // below fails fast with a clear selector timeout rather than flaking.
    await loginAsAdmin(page, baseURL)
    await gotoSandboxSettings(page, baseURL)

    // Device B — same admin, second context, fully loaded first so its
    // SandboxResourceLimits store is subscribed to sync:code_sandbox_settings.
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoSandboxSettings(pageB, baseURL)

      // pids_max is a plain InputNumber labelled "cgroup pids.max" (no unit
      // conversion, unlike the MiB↔bytes memory fields — so the on-screen
      // value maps 1:1 to the persisted integer). Read B's current value to
      // pick a distinct new one (valid range 8..=100000).
      const pidsB = byTestId(pageB, 'sandbox-rl-pids-max')
      await expect(pidsB).toBeVisible({ timeout: 15_000 })
      const beforePidsB = (await pidsB.inputValue()).trim()
      const newPids = beforePidsB === '128' ? 192 : 128

      // Mutate the singleton from device A's session via the REST endpoint
      // (PATCH semantics — only pids_max changes). The handler invalidates the
      // server cache AND publishes sync:code_sandbox_settings.
      const res = await page.request.put(
        `${baseURL}/api/code-sandbox/resource-limits`,
        {
          headers: await jsonHeaders(page),
          data: { pids_max: newPids },
        },
      )
      expect(res.ok()).toBeTruthy()

      // Device B reflects the new pids cap without a reload — the sync event
      // refetches the singleton and the section's useEffect re-seeds the form.
      await expect(pidsB).toHaveValue(String(newPids), { timeout: 15_000 })
    } finally {
      await ctxB.close()
    }
  })
})
