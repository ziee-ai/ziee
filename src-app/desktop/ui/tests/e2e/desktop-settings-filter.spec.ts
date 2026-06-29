import { test, expect } from '@playwright/test'
import { installTauriMock, mockBackendDefaults } from './helpers/tauri-mock'

test.describe('desktop settings filter', () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page)
    await mockBackendDefaults(page)
  })

  test('hides multi-user admin sections and collapses to a single flat menu', async ({
    page,
  }) => {
    await page.goto('/settings')

    // Wait for the menu to render — the desktop SettingsPage redirects
    // to the first available section, so something is always there.
    await expect(page.getByTestId('desktop-settings-menu')).toBeVisible({
      timeout: 10_000,
    })

    // Multi-user RBAC surfaces that have no role on a single-admin
    // desktop — hidden via HIDDEN_ITEMS in SettingsPage.tsx. Each
    // hidden section's menu item (derived id from its slot `path`)
    // must be absent.
    await expect(
      page.getByTestId('desktop-settings-menu-item-users'),
    ).toHaveCount(0)
    await expect(
      page.getByTestId('desktop-settings-menu-item-user-groups'),
    ).toHaveCount(0)
    await expect(
      page.getByTestId('desktop-settings-menu-item-auth-providers'),
    ).toHaveCount(0)

    // Assistant Templates: hidden because templates target a multi-
    // user fleet (a single admin can just create personal assistants
    // directly).
    await expect(
      page.getByTestId('desktop-settings-menu-item-assistant-templates'),
    ).toHaveCount(0)

    // User MCP page: hidden in favour of System MCP. The desktop's
    // AutoAssignMcpServerHandler propagates every new system server
    // to every group so the single admin sees it in chat without
    // any manual assignment step.
    await expect(
      page.getByTestId('desktop-settings-menu-item-mcp-servers'),
    ).toHaveCount(0)

    // The desktop SettingsPage builds menuItems as a flat list — no
    // section group entries. The kit Menu emits a
    // `${menuTestid}-group-${i}` testid on every group <li>; their
    // absence is the flat-list contract.
    await expect(
      page.getByTestId(/^desktop-settings-menu-group-/),
    ).toHaveCount(0)
  })

  test('keeps the user assistants page + the System MCP page visible (single-admin counterparts)', async ({
    page,
  }) => {
    await page.goto('/settings')

    await expect(page.getByTestId('desktop-settings-menu')).toBeVisible({
      timeout: 10_000,
    })

    // "Assistants" (the user-scope "My Assistants" slot) IS visible
    // — it's the single-admin counterpart to the now-hidden
    // "Assistant Templates" admin slot.
    await expect(
      page.getByTestId('desktop-settings-menu-item-assistants'),
    ).toHaveCount(1)

    // The System MCP page (slot path `mcp-admin`) IS visible — the
    // single-admin counterpart to the now-hidden user-scope
    // "MCP Servers" slot.
    await expect(
      page.getByTestId('desktop-settings-menu-item-mcp-admin'),
    ).toHaveCount(1)
  })

  test('keeps infrastructure admin sections visible', async ({ page }) => {
    await page.goto('/settings')

    await expect(page.getByTestId('desktop-settings-menu')).toBeVisible({
      timeout: 10_000,
    })

    // At least these should remain — single-admin still configures
    // infrastructure. (Slot paths: `llm-providers`, `sandbox`.)
    await expect(
      page.getByTestId('desktop-settings-menu-item-llm-providers'),
    ).toBeVisible()
    await expect(
      page.getByTestId('desktop-settings-menu-item-sandbox'),
    ).toBeVisible()
  })

  test('Memory shows exactly one entry that opens the combined page', async ({
    page,
  }) => {
    await page.goto('/settings')
    await expect(page.getByTestId('desktop-settings-menu')).toBeVisible({
      timeout: 10_000,
    })

    // Core registers TWO 'memory' slots (user + admin); the desktop
    // module registers ONE 'memory-desktop' (slot path `memory-combined`)
    // that filters both core entries. The menu must show exactly one
    // Memory entry.
    await expect(
      page.getByTestId('desktop-settings-menu-item-memory-combined'),
    ).toHaveCount(1)

    // Clicking lands on the desktop combined route.
    await page
      .getByTestId('desktop-settings-menu-item-memory-combined')
      .click()
    await expect(page).toHaveURL(/\/settings\/memory-combined\b/)

    // Both section headers from the combined page must be present.
    await expect(
      page.getByTestId('memory-combined-preferences-heading'),
    ).toBeVisible()
    await expect(
      page.getByTestId('memory-combined-administration-heading'),
    ).toBeVisible()
  })

  test('LLM Providers shows exactly one entry (admin page, no user-side dup)', async ({
    page,
  }) => {
    await page.goto('/settings')
    await expect(page.getByTestId('desktop-settings-menu')).toBeVisible({
      timeout: 10_000,
    })

    // Core registers BOTH a user-side slot ('user-llm-providers') and
    // an admin slot ('llm-providers'), both labeled 'LLM Providers'.
    // Desktop hides the user-side (no role on single-admin), so
    // exactly one LLM Providers entry (slot path `llm-providers`)
    // should appear.
    await expect(
      page.getByTestId('desktop-settings-menu-item-llm-providers'),
    ).toHaveCount(1)
  })
})
