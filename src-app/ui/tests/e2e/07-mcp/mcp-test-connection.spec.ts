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
 * E2E coverage for the MCP "Test Connection" feature:
 *  - the button inside the add/edit drawer (probes the current form values), and
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

  test('drawer reports a successful connection', async ({ page }) => {
    await openAddServerDrawer(page)
    await fillMcpServerForm(page, {
      name: 'test-conn-http-ok',
      displayName: 'Test Conn HTTP OK',
      transportType: 'http',
      url: mock.url(),
      enabled: true,
    })

    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await drawer.getByRole('button', { name: 'Test Connection' }).click()

    // Success is reported via an antd success message (does not close the drawer).
    await expect(page.locator('.ant-message-success')).toBeVisible({
      timeout: 10000,
    })
    await expect(drawer).toBeVisible()
  })

  test('drawer reports a failed connection', async ({ page }) => {
    await openAddServerDrawer(page)
    await fillMcpServerForm(page, {
      name: 'test-conn-http-fail',
      displayName: 'Test Conn HTTP Fail',
      transportType: 'http',
      // Port 1 is unbound → connection refused, fails fast.
      url: 'http://127.0.0.1:1/mcp',
      enabled: true,
    })

    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await drawer.getByRole('button', { name: 'Test Connection' }).click()

    await expect(page.locator('.ant-message-error')).toBeVisible({
      timeout: 10000,
    })
  })

  test('existing server card tests its connection', async ({ page }) => {
    // Create a reachable HTTP server first.
    await openAddServerDrawer(page)
    await fillMcpServerForm(page, {
      name: 'test-conn-card',
      displayName: 'Test Conn Card',
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

    const card = page.locator('.ant-card:has-text("Test Conn Card")').first()
    await card.locator('[data-testid="mcp-server-test-btn"]').click()

    await expect(page.locator('.ant-message-success')).toBeVisible({
      timeout: 10000,
    })
  })
})
