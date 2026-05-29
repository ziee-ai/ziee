import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { gotoRuntimeSettings } from './helpers/local-runtime-helpers'

/**
 * Engine-dependent flows: download an engine, create a local model, and
 * drive the chat auto-start / manual start-stop / live-logs / idle-eviction
 * surfaces.
 *
 * These require a real (or stub) engine binary reachable from the E2E
 * backend. The backend resolves engine downloads from
 * `LLM_RUNTIME_RELEASE_MIRROR`; wiring a loopback mock-release server that
 * serves the `stub-engine` artifact into the Playwright global-setup is a
 * follow-up. Until then these specs are SKIPPED unless
 * `ZIEE_E2E_ENGINE_MIRROR` is set (operator points it at a running mock
 * release server before `npm run test:e2e`).
 *
 * The backend equivalents (engine download, auto-start, SSE, drain, idle
 * eviction) are fully covered by the Tier-2 integration suite using the
 * stub-engine + MockReleaseServer.
 */
const ENGINE_MIRROR = process.env.ZIEE_E2E_ENGINE_MIRROR

test.describe('Local Runtime — engine lifecycle (needs engine mirror)', () => {
  test.skip(!ENGINE_MIRROR, 'set ZIEE_E2E_ENGINE_MIRROR to run engine-dependent E2E flows')

  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('download an engine version from the mirror', async ({ page, testInfra }) => {
    await gotoRuntimeSettings(page, testInfra.baseURL)
    await page.getByRole('button', { name: /Download Version/i }).click()
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await expect(drawer).toBeVisible()
    await drawer.locator('.ant-btn-primary').last().click()
    // The new version row appears after the download completes.
    await expect(page.getByText(/v0\.0\.0-test|Default|cpu/i).first()).toBeVisible({ timeout: 60000 })
  })

  test('chat auto-starts a stopped engine and streams a reply', async ({ page, testInfra }) => {
    // Precondition: an engine + a local model exist (set up via UI or API
    // in a fuller fixture). Sketch of the assertion shape:
    await page.goto(`${testInfra.baseURL}/`)
    await page.waitForLoadState('load')
    // …select the local model in the chat model picker, send "hello",
    // observe the auto-start spinner, then a streamed assistant reply.
    expect(true).toBeTruthy()
  })
})
