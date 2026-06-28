import { test, expect, type Page } from '@playwright/test'
import {
  FAKE_TOKENS,
  installTauriMock,
  mockBackendDefaults,
} from './helpers/tauri-mock'

// Single-admin desktop conditional UI on the System MCP servers page
// (audit c0ac6023d672 — `AppMode.store.multiUserMode=false` branch).
//
// `Stores.AppMode.multiUserMode` defaults to `true` (web build) and is
// flipped to `false` ONLY by the desktop UI bootstrap
// (`desktop/ui/src/main.tsx` → `setMultiUserMode(false)`). There is no
// server config / window seam to drive it from the web E2E harness, so
// the genuine platform branch is only reachable here, where the real
// desktop bundle boots with multiUserMode=false. We mock ONLY the
// backend HTTP boundary; the conditional-render logic in
// `SystemMcpServersPage.tsx` runs for real.
//
// On the single-admin desktop the page MUST:
//   - title "MCP Servers" (not "System MCP Servers" — the user/system
//     split doesn't exist),
//   - hide the admin `McpUserPolicyCard` (no meaningful audience), and
//   - hide every per-server `McpServerGroupsAssignmentCard` (no user
//     groups to assign to).

const SYSTEM_MCP_SERVER = {
  id: 'sys-mcp-fixture-1',
  name: 'fixture-system-server',
  display_name: 'Fixture System Server',
  description: 'System MCP server E2E fixture',
  transport_type: 'http',
  url: 'https://example.invalid/mcp',
  command: null,
  args: null,
  enabled: true,
  is_system: true,
  is_built_in: false,
  usage_mode: 'all',
  supports_sampling: false,
  timeout_seconds: 30,
  environment_variables: null,
  headers: null,
  environment_variables_entries: [],
  headers_entries: [],
  last_health_check_at: null,
  last_health_check_status: null,
  last_health_check_reason: null,
}

async function mockSystemMcpFixtures(page: Page) {
  // Registered AFTER mockBackendDefaults so these specific handlers win
  // over the `**/api/**` catch-all (which returns `[]`).

  // Without an explicit /api/auth/me, the catch-all `[]` overwrites the
  // FAKE_TOKENS-bootstrapped admin and the page renders "Not authorized".
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

  // One system server so a server card renders — making the per-server
  // GroupsAssignmentCard's ABSENCE a real assertion (it would render
  // were multiUserMode true).
  await page.route(/\/api\/mcp\/system-servers(\?|$)/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        servers: [SYSTEM_MCP_SERVER],
        total: 1,
        page: 1,
        per_page: 20,
      }),
    })
  })

  // The user-policy endpoint the McpUserPolicyCard would read if shown.
  await page.route(/\/api\/mcp\/user-policy(\?|$)/, async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        allowed_transports: ['http', 'stdio'],
        user_stdio_sandbox_flavor: 'full',
        tool_call_retention_days: 90,
      }),
    })
  })
}

test.describe('desktop single-admin — System MCP servers page', () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page)
    await mockBackendDefaults(page)
    await mockSystemMcpFixtures(page)
  })

  test('multiUserMode=false hides user-policy + per-server group assignment', async ({
    page,
  }) => {
    await page.goto('/settings/mcp-admin')

    // The fixture system server card renders (stable signal the page
    // loaded + the store hydrated). data-server-id is set per card.
    await expect(
      page.locator('[data-server-id="sys-mcp-fixture-1"]'),
    ).toBeVisible({ timeout: 15_000 })

    // Title is the single-admin "MCP Servers" — NOT "System MCP Servers"
    // (the multiUserMode ternary at SystemMcpServersPage.tsx:66).
    await expect(
      page.getByRole('heading', { name: 'MCP Servers', exact: true }),
    ).toBeVisible()
    await expect(
      page.getByRole('heading', { name: 'System MCP Servers' }),
    ).toHaveCount(0)

    // The admin user-policy card returns null when !multiUserMode
    // (McpUserPolicyCard.tsx:81) → its testid is absent.
    await expect(
      page.getByTestId('mcp-user-policy-card'),
    ).toHaveCount(0)

    // The per-server group-assignment card is wrapped in
    // `{multiUserMode && ...}` (SystemMcpServersPage.tsx:152) → absent
    // even though a server card IS present.
    await expect(
      page.locator('[data-card-type="user-groups-assignment"]'),
    ).toHaveCount(0)
  })
})
