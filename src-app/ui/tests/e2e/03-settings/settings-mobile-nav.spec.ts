import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — the Settings page mobile section-navigation dropdown.
 *
 * Audit gap (all-135116ed5efc): SettingsPage.tsx drives its layout
 * from its OWN container width (ResizeObserver via `useElementMinSize`).
 * Below the `sm` breakpoint (~640px) the desktop side Menu is replaced
 * by a Dropdown opened from a Button with aria-label="Select settings
 * section"; picking a menuitem calls `handleMenuClick(key)` →
 * `navigate('/settings/<key>')`. No spec covered this mobile branch.
 *
 * This forces the narrow layout with a small viewport, opens the
 * dropdown, switches from "General" to "Profile", and asserts the
 * route actually changed (the navigation, not just the menu paint).
 */

test.describe('Settings — mobile section navigation', () => {
  test('the mobile dropdown navigates between settings sections', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Narrow viewport BEFORE navigating so the settings container is
    // below `sm` and renders the mobile dropdown instead of the
    // desktop sidebar Menu.
    await page.setViewportSize({ width: 480, height: 900 })

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/general`)

    // Mobile layout active → the section-select Button is present
    // (the desktop sidebar Menu is not rendered at this width).
    const sectionButton = page.getByRole('button', {
      name: 'Select settings section',
    })
    await expect(sectionButton).toBeVisible({ timeout: 15_000 })

    // Open the dropdown and jump to the "Profile" section. Both
    // "General" and "Profile" are core user-level settings pages
    // always present for any authenticated user.
    await sectionButton.click()
    await page.getByRole('menuitem', { name: 'Profile' }).click()

    // The click must have driven a real route change.
    await expect(page).toHaveURL(/\/settings\/profile$/, { timeout: 10_000 })

    // The dropdown trigger reflects the now-current section.
    await expect(
      page.getByRole('button', { name: 'Select settings section' }),
    ).toContainText('Profile')
  })
})
