import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  goToMcpServersPage,
  waitForMcpPageLoad,
} from './helpers/navigation-helpers'
import {
  openAddServerDrawer,
  fillMcpServerForm,
  submitMcpServerForm,
} from './helpers/form-helpers'
import { MockSamplingServer } from './helpers/sampling-mock-server'

/**
 * E2E coverage for the MCP connection-test feature:
 *  - the "Save & Test Connection" button inside the add/edit drawer, which
 *    PERSISTS the entered settings first and then probes the stored server
 *    (flipping a fresh create into edit mode so a re-click updates rather than
 *    duplicating), and
 *  - the "Test" action on each existing server card.
 *
 * The in-process `MockSamplingServer` answers `initialize` + `tools/list`, so it
 * stands in as a reachable HTTP MCP server for the happy path; an unbound port
 * drives the failure path.
 */
test.describe('MCP - Test Connection', () => {
  let mock: MockSamplingServer

  test.beforeEach(async ({ page, testInfra }) => {
    mock = await MockSamplingServer.start()
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToMcpServersPage(page, baseURL)
    await waitForMcpPageLoad(page)
  })

  test.afterEach(async () => {
    await mock?.dispose()
  })

  test('drawer save & test reports a successful connection', async ({
    page,
  }) => {
    const tag = Date.now()
    await openAddServerDrawer(page)
    await fillMcpServerForm(page, {
      name: `test-conn-http-ok-${tag}`,
      displayName: `Test Conn HTTP OK ${tag}`,
      transportType: 'http',
      url: mock.url(),
      enabled: true,
    })

    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await drawer
      .getByRole('button', { name: 'Save & Test Connection' })
      .click()

    // Save + a successful probe both report via an antd success message, and
    // the drawer stays open (now in edit mode).
    await expect(page.locator('.ant-message-success').first()).toBeVisible({
      timeout: 10000,
    })
    await expect(drawer).toBeVisible()
  })

  test('drawer save & test reports a failed connection', async ({ page }) => {
    const tag = Date.now()
    await openAddServerDrawer(page)
    await fillMcpServerForm(page, {
      name: `test-conn-http-fail-${tag}`,
      displayName: `Test Conn HTTP Fail ${tag}`,
      transportType: 'http',
      // High unbound port: passes the form's URL validation (antd requires a
      // 2-5 digit port) so the save succeeds, but the probe fails fast (refused).
      url: 'http://127.0.0.1:59999/mcp',
      enabled: true,
    })

    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await drawer
      .getByRole('button', { name: 'Save & Test Connection' })
      .click()

    // The connection failure surfaces as an antd error message.
    await expect(page.locator('.ant-message-error')).toBeVisible({
      timeout: 10000,
    })
  })

  test('drawer save & test persists the server and transitions to edit', async ({
    page,
  }) => {
    const tag = Date.now()
    const displayName = `Save Test Persist ${tag}`
    await openAddServerDrawer(page)
    await fillMcpServerForm(page, {
      name: `save-test-persist-${tag}`,
      displayName,
      transportType: 'http',
      url: mock.url(),
      enabled: true,
    })

    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await drawer
      .getByRole('button', { name: 'Save & Test Connection' })
      .click()

    // The fresh create is persisted, so the drawer flips Add → Edit (and stays
    // open). The Name field is create-only, so its disappearance + the title
    // change both prove the transition.
    await expect(
      page.locator('.ant-drawer-title:has-text("Edit MCP Server")'),
    ).toBeVisible({ timeout: 10000 })
    await expect(page.locator('.ant-message-success').first()).toBeVisible({
      timeout: 10000,
    })
  })

  test('re-clicking save & test does not create a duplicate', async ({
    page,
  }) => {
    const tag = Date.now()
    const displayName = `Save Test NoDup ${tag}`
    await openAddServerDrawer(page)
    await fillMcpServerForm(page, {
      name: `save-test-nodup-${tag}`,
      displayName,
      transportType: 'http',
      url: mock.url(),
      enabled: true,
    })

    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    const saveAndTest = drawer.getByRole('button', {
      name: 'Save & Test Connection',
    })

    // First click creates the server and transitions the drawer to edit mode.
    await saveAndTest.click()
    await expect(
      page.locator('.ant-drawer-title:has-text("Edit MCP Server")'),
    ).toBeVisible({ timeout: 10000 })
    // Let the create/test toasts auto-dismiss so they can't mask the next round.
    await expect(page.locator('.ant-message-success').first()).toBeHidden({
      timeout: 8000,
    })

    // Second click now runs the UPDATE path (same server), not a second create.
    await saveAndTest.click()
    await expect(page.locator('.ant-message-success').first()).toBeVisible({
      timeout: 10000,
    })

    // Close the drawer and confirm exactly one card exists for this server.
    await drawer.getByRole('button', { name: 'Cancel' }).click()
    await expect(
      page.locator(`.ant-card:has-text("${displayName}")`),
    ).toHaveCount(1)
  })

  test('existing server card tests its connection', async ({ page }) => {
    const tag = Date.now()
    // Create a reachable HTTP server first (plain Save, which closes the drawer).
    await openAddServerDrawer(page)
    await fillMcpServerForm(page, {
      name: `test-conn-card-${tag}`,
      displayName: `Test Conn Card ${tag}`,
      transportType: 'http',
      url: mock.url(),
      enabled: true,
    })
    await submitMcpServerForm(page, 'create')

    // The creation success toast auto-dismisses; wait it out so the next
    // assertion can't match a stale message.
    await expect(page.locator('.ant-message-success')).toBeHidden({
      timeout: 6000,
    })

    const card = page
      .locator(`.ant-card:has-text("Test Conn Card ${tag}")`)
      .first()
    await card.locator('[data-testid="mcp-server-test-btn"]').click()

    await expect(page.locator('.ant-message-success')).toBeVisible({
      timeout: 10000,
    })
  })
})
