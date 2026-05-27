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
    await expect(
      page.getByRole('menuitem').first(),
    ).toBeVisible({ timeout: 10_000 })

    const menu = page.getByRole('menu')

    // These five labels correspond to settingsAdminPages entries with
    // ids in HIDDEN_ADMIN_ITEMS (users, user-groups, assistants,
    // mcp-admin, auth-providers).
    await expect(menu.getByText(/^Users$/)).toHaveCount(0)
    await expect(menu.getByText(/^User Groups$/)).toHaveCount(0)
    await expect(menu.getByText(/^Auth Providers$/)).toHaveCount(0)
    await expect(menu.getByText(/^Assistants$/)).toHaveCount(0)
    await expect(menu.getByText(/MCP.*Admin/i)).toHaveCount(0)

    // The desktop SettingsPage builds menuItems as a flat list — no
    // section group/divider entries. Antd renders groups as elements
    // with role="presentation" and class containing "menu-item-group";
    // their absence is the contract.
    const groupCount = await page
      .locator('.ant-menu-item-group')
      .count()
    expect(groupCount).toBe(0)
  })

  test('keeps infrastructure admin sections visible', async ({ page }) => {
    await page.goto('/settings')

    await expect(
      page.getByRole('menuitem').first(),
    ).toBeVisible({ timeout: 10_000 })

    const menu = page.getByRole('menu')

    // At least these should remain — single-admin still configures
    // infrastructure. (Labels match the module.tsx registrations.)
    await expect(menu.getByText(/LLM Providers/i)).toBeVisible()
    await expect(menu.getByText(/Code Sandbox/i)).toBeVisible()
  })

  test('Memory shows exactly one entry that opens the combined page', async ({
    page,
  }) => {
    await page.goto('/settings')
    await expect(
      page.getByRole('menuitem').first(),
    ).toBeVisible({ timeout: 10_000 })

    const menu = page.getByRole('menu')

    // Core registers TWO 'memory' slots (user + admin); the desktop
    // module registers ONE 'memory-desktop' that filters both core
    // entries. The menu must show exactly one "Memory".
    await expect(menu.getByText(/^Memory$/)).toHaveCount(1)

    // Clicking lands on the desktop combined route.
    await menu.getByText(/^Memory$/).click()
    await expect(page).toHaveURL(/\/settings\/memory-combined\b/)

    // Both section headers from the combined page must be present.
    await expect(page.getByText(/^Your preferences$/i)).toBeVisible()
    await expect(page.getByText(/^Administration$/i)).toBeVisible()
  })

  test('LLM Providers shows exactly one entry (admin page, no user-side dup)', async ({
    page,
  }) => {
    await page.goto('/settings')
    await expect(
      page.getByRole('menuitem').first(),
    ).toBeVisible({ timeout: 10_000 })

    const menu = page.getByRole('menu')

    // Core registers BOTH a user-side slot ('user-llm-providers') and
    // an admin slot ('llm-providers'), both labeled 'LLM Providers'.
    // Desktop hides the user-side (no role on single-admin), so
    // exactly one "LLM Providers" entry should appear.
    await expect(menu.getByText(/^LLM Providers$/)).toHaveCount(1)
  })
})
