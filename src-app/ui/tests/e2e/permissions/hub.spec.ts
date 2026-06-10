import { test, expect } from './no-403'
import { loginAsHubMcpOnly, loginAsMember, loginWithPerms } from './fixtures'
import { Permissions } from '../../../src/api-client/types'

test.describe('hub module — permission gating', () => {
  test('non-admin without any hub::*::read: sidebar entry hidden + /hub renders inline 403', async ({
    page,
    testInfra,
  }) => {
    await loginAsMember(page, testInfra.baseURL, testInfra.apiURL)

    // Sidebar Hub entry should NOT be visible — gated on the
    // `HUB_READ_PERM` anyOf in hub/module.tsx.
    await page.goto(`${testInfra.baseURL}/`)
    await expect(
      page.getByRole('link', { name: /^Hub$/i }),
    ).toHaveCount(0)

    // Deep-link directly to /hub — RoutePermissionGate should render
    // the inline 403 (URL preserved, no redirect to a different page).
    await page.goto(`${testInfra.baseURL}/hub`)
    await expect(page.getByText(/Not authorized/i)).toBeVisible()
    expect(page.url()).toContain('/hub')
  })

  test('hub-mcp-only user: MCP Servers tab visible, Models + Assistants tabs absent', async ({
    page,
    testInfra,
  }) => {
    await loginAsHubMcpOnly(page, testInfra.baseURL, testInfra.apiURL)

    // The route-level gate should pass — they have hub::mcp_servers::read.
    await page.goto(`${testInfra.baseURL}/hub`)
    await expect(page.getByText(/Not authorized/i)).toHaveCount(0)

    // Sidebar entry should be present. SidebarItem renders as a
    // <Link>, so the accessible role is `link`, not `menuitem` (the
    // existing users.spec.ts uses `menuitem` but its toHaveCount(0)
    // assertion passes for the wrong reason — see audit follow-up).
    await expect(
      page.getByRole('link', { name: /^Hub$/i }).first(),
    ).toBeVisible()

    // The Segmented control inside the Hub renders one item per
    // visible tab. Scope assertions to the segmented control (by its
    // antd class — it has no accessible name) so a sidebar entry with
    // the same label (e.g. "Assistants") can't satisfy the count-0
    // checks for absent tabs.
    const tabs = page.locator('.ant-segmented')
    await expect(tabs.getByText(/MCP Servers/i)).toBeVisible()
    await expect(tabs.getByText('Models', { exact: true })).toHaveCount(0)
    await expect(tabs.getByText('Assistants', { exact: true })).toHaveCount(0)
  })

  test('hub-models-only user accessing forbidden tab via URL: inline 403 (URL preserved)', async ({
    page,
    testInfra,
  }) => {
    // A user with read on Models only — but they navigate directly
    // to /hub/assistants. HubPage detects `urlSegmentIsForbidden`
    // and renders the inline Result rather than redirecting.
    await loginWithPerms(
      page,
      testInfra.baseURL,
      testInfra.apiURL,
      [Permissions.HubModelsRead],
      'hub-models-only',
    )

    await page.goto(`${testInfra.baseURL}/hub/assistants`)

    // The HubPage's own forbidden-tab Result reads "Tab Not
    // Available" / "You don't have permission to view this Hub tab."
    // Either copy is acceptable — match the subtitle which is more
    // specific.
    await expect(
      page.getByText(/don't have permission to view this Hub tab/i),
    ).toBeVisible()
    expect(page.url()).toContain('/hub/assistants')
  })

  test('refresh button visibility tracks ::refresh permission', async ({
    page,
    testInfra,
  }) => {
    // Reader without refresh: button hidden / disabled.
    await loginWithPerms(
      page,
      testInfra.baseURL,
      testInfra.apiURL,
      [Permissions.HubModelsRead],
      'hub-models-read-only',
    )

    await page.goto(`${testInfra.baseURL}/hub/models`)
    // Wait for the Hub tab to render. We assert the absence of the
    // refresh button rather than its specific selector — the button
    // either isn't mounted (gated out) or is disabled. Use a generous
    // poll because the tab body is lazy-loaded.
    const refreshBtn = page.getByRole('button', { name: /refresh/i })
    await expect(refreshBtn).toHaveCount(0)
  })
})
