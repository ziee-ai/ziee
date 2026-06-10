import { test, expect, type Page } from '@playwright/test'
import {
  FAKE_TOKENS,
  installTauriMock,
  mockBackendDefaults,
} from './helpers/tauri-mock'

// Desktop-specific hub-card affordances. On the single-admin desktop:
//   - Hub assistant card MUST hide the "Use as Template" button
//     (templates target a multi-user fleet — meaningless on a single-
//     user device). Only "Use Assistant" remains.
//   - Hub MCP card MUST hide the "Install for me" button (user MCP page
//     is hidden — every install is system-scope). Only
//     "Install for the system" remains.
//
// Both behaviours are gated on `Stores.AppMode.multiUserMode`, which
// the desktop UI bootstrap flips to `false` at startup. The default
// FAKE_TOKENS in tauri-mock already grants `permissions: ['*']` so
// every gate-on-permission button SHOULD render in the multi-user
// world — proving the platform branch is what hides them on desktop.

const HUB_ASSISTANT = {
  id: 'hub-assistant-fixture-1',
  name: 'test-assistant',
  display_name: 'Test Assistant',
  description: 'Hub assistant E2E fixture',
  author: 'fixture',
  category: 'general',
  tags: [],
  capabilities_required: [],
  example_prompts: [],
  recommended_models: [],
  popularity_score: 0,
  created_ids: [],
  created_template_ids: [],
  instructions: 'You are a test assistant.',
  parameters: null,
}

const HUB_MCP_SERVER = {
  id: 'hub-mcp-fixture-1',
  name: 'test-mcp-server',
  display_name: 'Test MCP Server',
  description: 'Hub MCP E2E fixture',
  author: 'fixture',
  category: 'general',
  tags: [],
  popularity_score: 0,
  transport_type: 'http',
  url: 'https://example.invalid/mcp',
  command: null,
  args: null,
  required_env: [],
  required_headers: [],
  environment_variables: null,
  headers: null,
  created_ids: [],
  created_system_ids: [],
  github_url: null,
  documentation_url: null,
  source_auth_configured: false,
}

const HUB_VERSION = {
  hub_version: '0.0.3-alpha',
  server_version: '0.1.0',
  counts: { models: 0, assistants: 1, mcp_servers: 1 },
  source: 'seed',
  installed_at: '2026-01-01T00:00:00Z',
}

async function mockHubFixtures(page: Page) {
  // Playwright route patterns: glob matching is exact against the
  // URL including the query string, so `**/api/hub/mcp-servers`
  // does NOT match `/api/hub/mcp-servers?lang=en`. We use regex
  // anchors that ignore the query so e.g. `getMCPServers({lang:'en'})`
  // is intercepted.
  //
  // Registered AFTER mockBackendDefaults so these (more specific)
  // handlers win the dispatch against the catch-all `**/api/**`.
  //
  // /api/auth/me — without this, the catch-all returning `[]`
  // overwrites the FAKE_TOKENS-bootstrapped user and the hub tab
  // renders the "Not authorized" panel.
  await page.route(/\/api\/auth\/me(\?|$)/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        user: FAKE_TOKENS.user,
        permissions: FAKE_TOKENS.user.permissions,
        has_password: true,
      }),
    })
  })
  await page.route(/\/api\/hub\/version(\?|$)/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(HUB_VERSION),
    })
  })
  await page.route(/\/api\/hub\/index(\?|$)/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        hub_version: HUB_VERSION.hub_version,
        assistants: [HUB_ASSISTANT],
        mcp_servers: [HUB_MCP_SERVER],
        models: [],
      }),
    })
  })
  await page.route(/\/api\/hub\/assistants(\?|$)/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([HUB_ASSISTANT]),
    })
  })
  await page.route(/\/api\/hub\/assistants\/version(\?|$)/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      // HubVersionResponse: { version, last_updated }
      body: JSON.stringify({
        version: HUB_VERSION.hub_version,
        last_updated: '2026-01-01T00:00:00Z',
      }),
    })
  })
  await page.route(/\/api\/hub\/mcp-servers(\?|$)/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([HUB_MCP_SERVER]),
    })
  })
  await page.route(/\/api\/hub\/mcp-servers\/version(\?|$)/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        version: HUB_VERSION.hub_version,
        last_updated: '2026-01-01T00:00:00Z',
      }),
    })
  })
  // The mcp user-policy endpoint is read by the hub MCP tab's
  // shouldRender gate. Allow it (admin can add anything).
  await page.route(/\/api\/mcp\/user-policy(\?|$)/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        allowed_transports: ['http', 'stdio'],
        user_stdio_sandbox_flavor: 'full',
      }),
    })
  })
}

test.describe('desktop hub card buttons', () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page)
    await mockBackendDefaults(page)
    await mockHubFixtures(page)
  })

  test('hub assistant card shows only "Use Assistant" (no "Use as Template") on desktop', async ({
    page,
  }) => {
    await page.goto('/hub/assistants')

    // Wait for the card to render — the display_name is the stable
    // signal that the assistant fixture made it through the store.
    await expect(
      page.getByText('Test Assistant').first(),
    ).toBeVisible({ timeout: 15_000 })

    // Use Assistant button MUST be present (this is the surviving
    // affordance after the desktop strip).
    await expect(
      page.getByTestId('hub-assistant-use-btn'),
    ).toBeVisible()

    // Use as Template MUST NOT render on desktop — the JSX is wrapped
    // in `Stores.AppMode.multiUserMode` which the desktop bootstrap
    // sets to false.
    await expect(
      page.getByTestId('hub-assistant-use-as-template-btn'),
    ).toHaveCount(0)
  })

  test('hub MCP card shows only "Install for the system" (no "Install for me") on desktop', async ({
    page,
  }) => {
    await page.goto('/hub/mcp-servers')

    await expect(
      page.getByText('Test MCP Server').first(),
    ).toBeVisible({ timeout: 15_000 })

    // Install for the system MUST remain — it's the surviving
    // affordance on desktop where every install is system-scope.
    await expect(
      page.getByTestId('hub-mcp-install-as-system-btn'),
    ).toBeVisible()

    // Install for me (user-scope) MUST be hidden. Same multiUserMode
    // gate as the assistant case. The "View Server" button (which
    // shares the same JSX branch when `isAlreadyInstalled === true`)
    // is also wrapped by the same gate — the fixture has empty
    // created_ids so isAlreadyInstalled is false anyway, but the
    // gate would hide it too.
    await expect(
      page.getByTestId('hub-mcp-install-btn'),
    ).toHaveCount(0)
    await expect(page.getByTestId('hub-mcp-view-btn')).toHaveCount(0)
  })
})
